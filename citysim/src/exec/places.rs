use super::*;

impl World {
    /// The building a symbolic location refers to for this agent, if any.
    pub fn resolve_building(&self, agent: EntityId, key: LocationKey, target: Option<EntityId>) -> Option<EntityId> {
        use crate::components::BuildingKind as K;
        match key {
            LocationKey::Home => self.comp::<Household>(agent).and_then(|h| h.home),
            LocationKey::TargetHome => target,
            LocationKey::Farm => target
                .filter(|&t| self.comp::<Building>(t).is_some_and(|b| b.kind == K::Farm))
                .or_else(|| self.comp::<crate::components::Job>(agent).and_then(|j| j.employer))
                .filter(|&t| self.comp::<Building>(t).is_some_and(|b| b.kind == K::Farm))
                .or_else(|| self.local(agent, K::Farm)),
            // M10: several of each; the agent's own (D21).
            LocationKey::Market => target.or_else(|| self.local(agent, K::Market)),
            LocationKey::Bar => target.or_else(|| self.local(agent, K::Bar)),
            // Lives at work phase 7 (§ 5.2): an escort's cells (the nearest
            // of the Precinct and the Stations with a free cell), a Station
            // guard's post; else the Precinct.
            LocationKey::Jail => target.or_else(|| crate::systems::law::jail_for(self, agent)),
            LocationKey::Cemetery => target.or_else(|| self.building_of_kind(K::Cemetery)),
            LocationKey::Hall => target.or_else(|| self.building_of_kind(K::Hall)),
            LocationKey::Hideout => target.or_else(|| crate::systems::gang::hideout_for(self, agent)),
            LocationKey::Warehouse => target.or_else(|| self.building_of_kind(K::Warehouse)),
            // A suspect or corpse inside a building is reached through its door.
            LocationKey::SuspectTile | LocationKey::CorpseTile => {
                target.and_then(|s| self.comp::<Position>(s)).and_then(|p| p.building)
            }
            LocationKey::PatrolWaypoint => self
                .comp::<Brain>(agent)
                .and_then(|b| b.patrol_route.get(usize::from(b.patrol_legs) % b.patrol_route.len().max(1)).copied()),
            // M11 D13: the wage desk (the Hall for a city job).
            LocationKey::Workplace => target
                .filter(|&t| self.comp::<Building>(t).is_some_and(|b| b.kind == K::SecurityOffice))
                // L2: a venue's or Fab's staff work at their employer (a
                // city-owned one's wage desk is the Hall).
                .or_else(|| {
                    self.comp::<crate::components::Job>(agent)
                        .and_then(|j| j.employer)
                        // The Real economy E26: a Mission's Volunteers work
                        // at it (a city deed's wage desk would be the Hall).
                        .filter(|&e| {
                            crate::systems::jobs::is_l2_building(self, e)
                                || crate::systems::charity::is_mission(self, e)
                                // Jobs and room J13: a trade's staff work at
                                // their employer, whoever pays.
                                || crate::systems::trades::at_post(self, agent, e)
                        })
                })
                .or_else(|| self.wage_desk(agent)),
            // M12 D21: the booked Hotel, else the one the agent can reach and pay.
            // OPEN-140: a housed sleeper's Hotel is the planner's
            // (`life::away_hotel`: a Hotel nearer than Home), never the
            // homeless's nearest cot or a bed out of the planner's reach
            // ("Hotel" resolved to a Soup Kitchen, or to none: NoSuchPlace).
            LocationKey::Hotel
                if self.comp::<crate::components::Household>(agent).is_some_and(|h| h.home.is_some()) =>
            {
                target
                    .filter(|&t| {
                        crate::systems::street::is_hotel(self, t) && !crate::systems::charity::is_cot_house(self, t)
                    })
                    .or_else(|| crate::systems::life::away_hotel(self, agent))
            }
            // The island pass phase 2 (OPEN-171): with `[distance.free]` on, a
            // "Hotel" is a Hotel and a cot is `Cot`; a pick that turned from
            // one to the other since the plan is no such place (a replan).
            LocationKey::Hotel if crate::systems::distance::free(self).is_some() => target
                .filter(|&t| crate::systems::street::is_hotel(self, t) && !crate::systems::charity::is_mission(self, t))
                .or_else(|| {
                    crate::systems::street::hotel_for(self, agent)
                        .filter(|&h| !crate::systems::charity::is_mission(self, h))
                }),
            LocationKey::Hotel => target
                .filter(|&t| crate::systems::street::is_hotel(self, t))
                .or_else(|| crate::systems::street::hotel_for(self, agent)),
            LocationKey::Cot => target.filter(|&t| crate::systems::charity::is_cot_house(self, t)).or_else(|| {
                crate::systems::street::hotel_for(self, agent).filter(|&h| crate::systems::charity::is_mission(self, h))
            }),
            // M12 D27: the agent's squat, else the bound derelict.
            LocationKey::Squat => self
                .comp::<crate::components::Squatter>(agent)
                .map(|s| s.building)
                .or_else(|| target.filter(|&t| crate::systems::street::is_derelict(self, t))),
            // M12 D37: inside the Hideout; a door muster resolves to its tile.
            LocationKey::MusterPoint => match crate::systems::raid::muster_point(self, agent) {
                Some(crate::systems::raid::MusterAt::Inside(b)) => Some(b),
                _ => None,
            },
            // M13 D29: the bound seller; D47: a bound Garage, else the nearest.
            LocationKey::Seller => target.filter(|&t| self.has::<Building>(t)),
            LocationKey::Garage => {
                let garage = |t: &EntityId| self.comp::<Building>(*t).is_some_and(|b| b.kind == K::Garage);
                target
                    .filter(garage)
                    .or_else(|| self.comp::<crate::components::Job>(agent).and_then(|j| j.employer).filter(garage))
                    .or_else(|| self.local(agent, K::Garage))
            }
            // M13 D34: the bound Clinic, else a Ripperdoc's employer, else the nearest.
            // Pressure pass R3 (OPEN-216): a bound back room is walked to (a
            // care trip or a Rescue bound to a BlackClinic resolved to the
            // nearest Clinic, and its Patch there failed `PreconditionLost`).
            LocationKey::Clinic => {
                let clinic = |t: &EntityId| self.comp::<Building>(*t).is_some_and(|b| b.kind == K::Clinic);
                let back = |t: &EntityId| self.comp::<Building>(*t).is_some_and(|b| b.kind == K::BlackClinic);
                target
                    .filter(|t| clinic(t) || back(t))
                    .or_else(|| self.comp::<crate::components::Job>(agent).and_then(|j| j.employer).filter(clinic))
                    .or_else(|| self.local(agent, K::Clinic))
            }
            // M13 D38/D40: the bound Stims source (a dealer's Bar, a legal Market).
            LocationKey::StimSource => target.filter(|&t| self.has::<Building>(t)),
            // M13 D36: a quarry inside a building is reached through its door.
            LocationKey::Victim => target.and_then(|s| self.comp::<Position>(s)).and_then(|p| p.building),
            // M14 V5: the chair of the agent's run order.
            LocationKey::Chair => self.run_orders.get(&agent).map(|o| o.chair),
            // M14 V29: the bound Lab of a Data buyer, else the nearest one.
            LocationKey::DataBuyer => target
                .filter(|&t| self.comp::<Building>(t).is_some_and(|b| b.kind == K::Lab))
                .or_else(|| crate::systems::tech::data_buyer_lab(self, agent)),
            // M15 W19: the Hunt's venue, then its intel building.
            LocationKey::Intel => crate::systems::hunt::intel_building(self, agent),
            // M13 D26: a vehicle is reached on the street outside its door.
            LocationKey::Anywhere
            | LocationKey::Street
            | LocationKey::RaidTarget
            | LocationKey::Vehicle
            | LocationKey::Beat
            | LocationKey::Spot
            | LocationKey::Bin => None,
            // The pre-view pass phase 2: a stations spot's building.
            // M18 P1: a picked building.
            LocationKey::Station | LocationKey::Picked => target.filter(|&t| self.has::<Building>(t)),
        }
    }

    /// The tile a symbolic location resolves to: a building's door, or a
    /// street tile. `None` means the place does not exist for this agent.
    pub fn resolve_location(&self, agent: EntityId, key: LocationKey, target: Option<EntityId>) -> Option<TilePos> {
        if let Some(b) = self.resolve_building(agent, key, target) {
            return self.comp::<Building>(b).map(|bd| bd.door);
        }
        match key {
            LocationKey::Street | LocationKey::Anywhere => {
                let pos = self.comp::<Position>(agent)?;
                match pos.building.and_then(|b| self.comp::<Building>(b)) {
                    Some(b) => Some(self.outside_door(b)),
                    None => Some(pos.tile),
                }
            }
            LocationKey::SuspectTile => target.and_then(|t| self.last_seen().get(&t)).map(|&(tile, _)| tile),
            LocationKey::CorpseTile | LocationKey::Victim => {
                target.and_then(|t| self.comp::<Position>(t)).map(|p| p.tile)
            }
            LocationKey::RaidTarget => crate::systems::raid::target_tile(self, agent),
            LocationKey::MusterPoint => match crate::systems::raid::muster_point(self, agent) {
                Some(crate::systems::raid::MusterAt::Door(t)) => Some(t),
                _ => None,
            },
            LocationKey::Vehicle => target.and_then(|v| crate::systems::vehicles::vehicle_stand(self, v)),
            // M15 W19: a street intel (a database sighting's tile).
            LocationKey::Intel => crate::systems::hunt::intel_tile(self, agent),
            // L2 L8: the beat's dirtiest street tile.
            LocationKey::Beat => crate::systems::jobs::beat_tile(self, agent),
            // L2 L14: the leisure pick's street spot.
            LocationKey::Spot => crate::systems::leisure::spot_of(self, agent),
            // Hunger as sickness § 4: the bin outside the bound building's door.
            LocationKey::Bin => target.and_then(|b| crate::systems::bins::bin_tile(self, b)),
            _ => None,
        }
    }

    /// Stand outside a building's door (off its occupant list). Promotion,
    /// demotion and coming of age all place agents this way.
    pub fn stand_at_door(&mut self, agent: EntityId, b: EntityId) {
        let Some(door) = self.comp::<Building>(b).map(|bd| bd.door) else { return };
        self.remove_from_building(agent);
        let tick = self.tick;
        if let Some(p) = self.comp_mut::<Position>(agent) {
            p.tile = door;
            p.building = None;
            p.floor = 0;
            p.entered = tick;
        }
    }

    /// Guards enter the Jail whatever its occupancy: the prisoner cap is the
    /// law system's rule (fines and early releases), not the door's.
    pub fn capacity_exempt(&self, agent: EntityId, b: EntityId) -> bool {
        // Lives at work phase 7: and at a Station (its desk and its cells).
        let guard_at_jail = self.comp::<Building>(b).is_some_and(|bd| {
            matches!(bd.kind, crate::components::BuildingKind::Jail | crate::components::BuildingKind::Station)
        }) && crate::systems::law::is_guard(self, agent);
        // Round 8 X1 (`OPEN-246`): a witness come to the desk to give his
        // word (ten minutes at the counter): a Station or a Hall at its
        // cap does not turn a reporter away (a ReportCrime plan, any witness).
        let at_desk = self.comp::<Building>(b).is_some_and(|bd| {
            matches!(
                bd.kind,
                crate::components::BuildingKind::Jail
                    | crate::components::BuildingKind::Station
                    | crate::components::BuildingKind::Hall
            )
        }) && self.comp::<Brain>(agent).and_then(|x| x.plan_goal())
            == Some(crate::components::GoalKind::ReportCrime);
        // Residents always get into their own Home (births may exceed the cap).
        let own_home = self.comp::<Household>(agent).and_then(|h| h.home) == Some(b);
        // M12: a booked guest gets into their Hotel, a squatter into the
        // squat, a sweeper into the Recycler it reports to.
        let own_bed = crate::systems::street::booked_hotel(self, agent) == Some(b)
            || self.comp::<crate::components::Squatter>(agent).is_some_and(|s| s.building == b);
        let sweeper = self
            .comp::<crate::components::Job>(agent)
            .is_some_and(|j| j.role == crate::components::Role::Sanitation && j.employer == Some(b));
        // Workplace room: with `[exec] staff_door`, any staff into their
        // own workplace (the shift's room is the jobs round's to keep).
        let staff = self.config.exec.staff_door
            && self.comp::<crate::components::Job>(agent).is_some_and(|j| j.employer == Some(b));
        // Jobs and room J13: a trade's staff into their workplace, a Super
        // into the Blocks of his round.
        guard_at_jail
            || at_desk
            || own_home
            || own_bed
            || sweeper
            || staff
            || crate::systems::trades::capacity_exempt(self, agent, b)
            || crate::systems::crew::capacity_exempt(self, agent, b)
            || crate::systems::charity::capacity_exempt(self, agent, b)
    }

    /// The street tile just outside a building's door (a Road if there is one).
    pub fn outside_door(&self, b: &Building) -> TilePos {
        let mut fallback = None;
        for n in self.map.neighbours4(b.door) {
            if b.rect.contains(n) || !self.map.walkable(n) {
                continue;
            }
            if self.map.tile_at(n) == crate::components::TileKind::Road {
                return n;
            }
            fallback.get_or_insert(n);
        }
        fallback.unwrap_or(b.door)
    }

    /// Put the agent inside `b` on the free interior tile nearest its door
    /// (S1, `OPEN-307`: the first free tile row-major from the rect's
    /// north-west corner put a big building's newcomer 9-23 tiles from the
    /// door it came through, crossed in one tick both ways).
    pub fn enter_building(&mut self, agent: EntityId, b: EntityId) {
        let Some(bd) = self.comp::<Building>(b) else { return };
        let mut taken: Vec<TilePos> = bd
            .occupants
            .iter()
            .filter(|&&o| o != agent)
            .filter_map(|&o| self.comp::<Position>(o).map(|p| p.tile))
            .collect();
        taken.sort_unstable();
        let slot = bd.slot_by_door(&taken).unwrap_or(bd.door);
        if let Some(bd) = self.comp_mut::<Building>(b) {
            if let Err(i) = bd.occupants.binary_search(&agent) {
                bd.occupants.insert(i, agent);
            }
        }
        let tick = self.tick;
        // The city milestone C6a: his flat at home (dealt on the first
        // entry), the storey he stands on.
        crate::systems::floors::assign_flat(self, agent);
        let floor = crate::systems::floors::floor_in(self, agent, b);
        if let Some(p) = self.comp_mut::<Position>(agent) {
            p.tile = slot;
            p.building = Some(b);
            p.entered = tick;
            p.floor = floor;
        }
        // OPEN-140: home at last.
        if self.moving_in.get(&agent) == Some(&b) {
            self.moving_in.remove(&agent);
        }
    }

    /// Step out onto the street outside the current building, if inside one.
    pub fn leave_building(&mut self, agent: EntityId) {
        let Some(here) = self.comp::<Position>(agent).and_then(|p| p.building) else { return };
        let outside = self.comp::<Building>(here).map(|bd| self.outside_door(bd));
        self.remove_from_building(agent);
        if let (Some(p), Some(out)) = (self.comp_mut::<Position>(agent), outside) {
            p.tile = out;
            p.building = None;
            p.floor = 0;
        }
    }

    /// Lazily build and cache the flow field for a building's door; past
    /// `[exec] flow_field_cache` fields (Scale S3a: or the
    /// `flow_field_budget_mb` bytes) the least recently used is evicted.
    pub fn flow_field_for(&mut self, b: EntityId) -> Option<&FlowField> {
        if !self.flow_fields.contains(b) {
            let door = self.comp::<Building>(b)?.door;
            let field = FlowField::build(&self.map, door);
            let e = &self.config.exec;
            let cap = FlowCache::cap_for(e.flow_field_cache, e.flow_field_budget_mb << 20, field.next.len());
            self.flow_fields.insert(b, field, cap);
        }
        self.flow_fields.get(b)
    }

    /// Scale S3a: does a Full walk on this map follow exact A* paths
    /// (`exec::bigpath`) rather than per-door flow fields? Past `[exec]
    /// flow_field_max_tiles` tiles.
    pub fn big_paths(&self) -> bool {
        self.map.w() * self.map.h() > self.config.exec.flow_field_max_tiles
    }

    /// Scale S3a: the shortest path from `from` (exclusive) to `to`
    /// (inclusive), forward, or `None` with no way there.
    pub fn big_path(&self, from: TilePos, to: TilePos) -> Option<Vec<TilePos>> {
        self.big_route(from, 0, to, true).map(|(p, _)| p)
    }

    /// The city milestone C7: [`World::big_path`] from `from` at `level` (a
    /// walkway's deck above 0), over the walkway portals when on `foot` (the
    /// escalator's direction at this tick), with each step's level (empty:
    /// every step on the ground).
    pub fn big_route(&self, from: TilePos, level: u8, to: TilePos, foot: bool) -> Option<(Vec<TilePos>, Vec<u8>)> {
        let labels = self.flow_fields.labels(&self.map);
        // V7: a driver's search has its own landmarks (the walkers' crossing costs are not its).
        let marks =
            if foot { self.flow_fields.landmarks(&self.map) } else { self.flow_fields.landmarks_drive(&self.map) };
        let portals = self.flow_fields.portals();
        let none = bigpath::Portals::none();
        let p = if foot { &*portals } else { &none };
        let minute = (self.tick % crate::time::TICKS_PER_DAY) as u32;
        crate::prof::timed!(
            "exec::big_path",
            bigpath::find_with(&self.map, &labels, &marks, p, from, level, to, minute)
        )
    }

    /// C7: the walkway portal's edge a walker hops from `from` to `to` (tile
    /// and level each) that runs at this tick, if any.
    pub fn portal_edge(&self, from: (TilePos, u8), to: (TilePos, u8)) -> Option<bigpath::Edge> {
        let portals = self.flow_fields.portals();
        if portals.is_empty() {
            return None;
        }
        let minute = (self.tick % crate::time::TICKS_PER_DAY) as u32;
        portals.edge(&self.map, from, to, minute).copied()
    }

    /// Next tile toward building `b` from `from`.
    pub fn flow_step(&mut self, b: EntityId, from: TilePos) -> Option<TilePos> {
        self.flow_field_for(b).and_then(|f| f.step(from))
    }

    /// Drop every cached field (after BuildHome / DemolishHome).
    pub fn invalidate_flow_fields(&mut self) {
        self.flow_fields.clear();
        self.flow_fields.keep_landmarks_if(&self.map);
        self.note_landmark_base();
    }

    /// Review finding 10: what a save carries of the landmarks (their tiles
    /// while a closure left them older than the map's).
    fn note_landmark_base(&mut self) {
        self.landmark_base = self.flow_fields.stale_base(&self.map).map(bigpath::BaseTiles);
    }

    /// A building went up on a Lot (`rect`, its door `door`): drop the
    /// cached fields whose paths from outside cross its new walls or its
    /// interior, and reseal the rest (OPEN-112: each kept field is then
    /// byte-identical to a rebuild).
    pub fn invalidate_flow_fields_for_lot(&mut self, rect: crate::components::Rect, door: TilePos) {
        let tiles: Vec<TilePos> = (rect.y..rect.y + rect.h)
            .flat_map(|y| (rect.x..rect.x + rect.w).map(move |x| TilePos { x, y }))
            .filter(|&t| self.map.tile_at(t) != crate::components::TileKind::Door)
            .collect();
        let sealed = |t: TilePos| rect.contains(t) && self.map.tile_at(t) != crate::components::TileKind::Door;
        self.flow_fields.invalidate_sealed(&self.map, rect, door, &tiles, &sealed);
        self.flow_fields.keep_landmarks_if(&self.map);
        self.note_landmark_base();
    }
}
