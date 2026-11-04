use super::*;

impl World {
    pub(crate) fn after_insert<T: Component>(&mut self, id: EntityId) {
        use std::any::TypeId;
        if TypeId::of::<T>() == TypeId::of::<Brain>() {
            self.index_brain(id);
            self.mark_claims_brain(id.index);
        } else if TypeId::of::<T>() == TypeId::of::<Job>() {
            self.index_job(id);
        } else if TypeId::of::<T>() == TypeId::of::<Gang>() {
            Self::list_insert(&mut self.gang_ids, id);
        } else if TypeId::of::<T>() == TypeId::of::<Sentence>() {
            Self::list_insert(&mut self.sentenced_ids, id);
        } else if TypeId::of::<T>() == TypeId::of::<Corp>() {
            Self::list_insert(&mut self.corp_ids, id);
        } else if TypeId::of::<T>() == TypeId::of::<Household>() {
            self.index_household(id);
        } else if TypeId::of::<T>() == TypeId::of::<Squatter>() {
            self.index_squatter(id);
        } else if TypeId::of::<T>() == TypeId::of::<Building>() {
            self.building_gen += 1;
        } else if TypeId::of::<T>() == TypeId::of::<Position>() {
            self.pos_index.mark(id.index);
        }
        if Self::shop_store::<T>() {
            self.shop_gen += 1;
        }
    }

    /// The perf pass phase 4: a store the price-keyed caches read (a write
    /// to one bumps `shop_gen`; the type test is resolved at compile time).
    pub(crate) fn shop_store<T: Component>() -> bool {
        use std::any::TypeId;
        let t = TypeId::of::<T>();
        t == TypeId::of::<Building>() || t == TypeId::of::<Corp>() || t == TypeId::of::<Market>()
    }

    pub(crate) fn after_remove<T: Component>(&mut self, id: EntityId) {
        use std::any::TypeId;
        if TypeId::of::<T>() == TypeId::of::<Brain>() {
            self.unindex_brain(id);
            self.mark_claims_brain(id.index);
        } else if TypeId::of::<T>() == TypeId::of::<Job>() {
            self.unindex_job(id);
        } else if TypeId::of::<T>() == TypeId::of::<Gang>() {
            self.unindex_gang(id);
        } else if TypeId::of::<T>() == TypeId::of::<Sentence>() {
            self.unindex_sentenced(id);
        } else if TypeId::of::<T>() == TypeId::of::<Corp>() {
            self.unindex_corp(id);
        } else if TypeId::of::<T>() == TypeId::of::<Household>() {
            self.unindex_household(id);
        } else if TypeId::of::<T>() == TypeId::of::<Squatter>() {
            self.unindex_squatter(id);
        } else if TypeId::of::<T>() == TypeId::of::<Building>() {
            self.building_gen += 1;
        } else if TypeId::of::<T>() == TypeId::of::<Position>() {
            self.pos_index.mark(id.index);
        }
        if Self::shop_store::<T>() {
            self.shop_gen += 1;
        }
    }

    /// File a squatter under its building (D27).
    fn index_squatter(&mut self, id: EntityId) {
        self.unindex_squatter(id);
        if let Some(b) = self.comp::<Squatter>(id).map(|s| s.building) {
            Self::list_insert(self.squat_index.entry(b).or_default(), id);
        }
    }

    /// Drop an agent from every squat list (its Squatter removed, or despawned).
    pub(crate) fn unindex_squatter(&mut self, id: EntityId) {
        self.squat_index.retain(|_, list| {
            Self::list_remove(list, id);
            !list.is_empty()
        });
    }

    /// The squatters of a building, ascending.
    pub fn squatters_of(&self, b: EntityId) -> &[EntityId] {
        self.squat_index.get(&b).map_or(&[], Vec::as_slice)
    }

    /// File the agent under its Household's Home.
    fn index_household(&mut self, id: EntityId) {
        self.unindex_household(id);
        if let Some(home) = self.comp::<Household>(id).and_then(|h| h.home) {
            Self::list_insert(self.residents.entry(home).or_default(), id);
            self.resident_home.insert(id, home);
        }
    }

    pub(crate) fn unindex_household(&mut self, id: EntityId) {
        if let Some(home) = self.resident_home.remove(&id) {
            if let Some(list) = self.residents.get_mut(&home) {
                Self::list_remove(list, id);
                if list.is_empty() {
                    self.residents.remove(&home);
                }
            }
        }
    }

    /// Move an agent into a Home (or out of any): the one write to
    /// `Household::home`, so `residents_of` stays in step.
    pub fn set_home(&mut self, id: EntityId, home: Option<EntityId>) {
        if let Some(h) = self.comp_mut::<Household>(id) {
            // M11 phase 5: losing a Home (eviction, demolition) ends its rent:
            // arrears kept after a DemolishHome barred re-housing for good.
            if home.is_none() {
                h.arrears = 0;
                h.rent_due = 0.0;
            } else {
                h.homeless_since = None;
            }
            // M18 P5h: a Home of his own lost is his address history (a corp reads it).
            let left = home.is_none() && h.home.is_some();
            // Round 6 V1 (OPEN-231): a move leaves his pantry stake behind
            // (the units stay on the old shelf, common there).
            if h.home != home {
                h.pantry_stake = 0.0;
                h.kin = None;
                h.flat = None;
            }
            h.home = home;
            self.index_household(id);
            if left {
                self.housing.left_home.insert(id, self.tick);
            }
        }
        // The city milestone C6a: a flat in the new Home's span.
        if home.is_some() {
            crate::systems::floors::assign_flat(self, id);
        }
        // OPEN-140: a walk home to a Home no longer his ends.
        if self.moving_in.get(&id).is_some_and(|&h| Some(h) != home) {
            self.moving_in.remove(&id);
        }
        // Fix pass (phase 3 review): any path to a Home (re-housing, a
        // marriage, a god command) ends a squat and tonight's Hotel booking;
        // a housed agent kept the slot and the Sweep evicted them.
        if home.is_some() {
            self.remove::<crate::components::Squatter>(id);
            self.hotel_beds.remove(&id);
            // M18 P5h: and the capsule and the city's list.
            self.housing.capsules.remove(&id);
            self.housing.list.remove(&id);
        }
    }

    /// Everyone whose Household names this Home, ascending (the living:
    /// a death removes the Household).
    pub fn residents_of(&self, home: EntityId) -> &[EntityId] {
        self.residents.get(&home).map_or(&[], Vec::as_slice)
    }

    /// Drop an agent from `sentenced_ids` (its Sentence removed, or despawned).
    pub(crate) fn unindex_sentenced(&mut self, id: EntityId) {
        Self::list_remove(&mut self.sentenced_ids, id);
    }

    /// Every agent with a `Sentence`, ascending by id (`with::<Sentence>()`
    /// without the entity scan).
    pub fn sentenced(&self) -> &[EntityId] {
        &self.sentenced_ids
    }

    /// Drop a gang from `gang_ids` (its Gang removed, or despawned).
    pub(crate) fn unindex_gang(&mut self, id: EntityId) {
        if self.gang_ids.binary_search(&id).is_ok() {
            // M14 V4: a gang gone (its Hideout node) relinks the plane.
            self.virt_dirty = true;
        }
        Self::list_remove(&mut self.gang_ids, id);
    }

    /// Drop a corp from `corp_ids` (its Corp removed, or despawned).
    pub(crate) fn unindex_corp(&mut self, id: EntityId) {
        if self.corp_ids.binary_search(&id).is_ok() {
            // M14 V4: a corp gone (its Ledger node) relinks the plane.
            self.virt_dirty = true;
        }
        Self::list_remove(&mut self.corp_ids, id);
    }

    fn list_remove(list: &mut Vec<EntityId>, id: EntityId) {
        if let Ok(i) = list.binary_search(&id) {
            list.remove(i);
        }
    }

    fn list_insert(list: &mut Vec<EntityId>, id: EntityId) {
        if let Err(i) = list.binary_search(&id) {
            list.insert(i, id);
        }
    }

    /// File the agent under its Brain's current LOD.
    pub fn index_brain(&mut self, id: EntityId) {
        self.unindex_brain(id);
        if let Some(b) = self.comp::<Brain>(id) {
            let lod = b.lod;
            Self::list_insert(&mut self.by_tier[lod as usize], id);
            if lod == Lod::Statistical {
                Self::list_insert(&mut self.stat_slots.0[StatSlots::slot_of(id)], id);
            }
        }
    }

    pub fn unindex_brain(&mut self, id: EntityId) {
        for list in &mut self.by_tier {
            Self::list_remove(list, id);
        }
        Self::list_remove(&mut self.stat_slots.0[StatSlots::slot_of(id)], id);
    }

    /// Call after any write to `Brain::lod`.
    pub fn retier(&mut self, id: EntityId) {
        self.index_brain(id);
    }

    /// File the agent under its Job's role (and, J3, its workplace).
    pub fn index_job(&mut self, id: EntityId) {
        self.unindex_job(id);
        self.mark_claims_job(id.index);
        if let Some(j) = self.comp::<Job>(id) {
            let (r, employer) = (role_index(j.role), j.employer);
            if r >= self.by_role.len() {
                self.by_role.resize(r + 1, Vec::new());
            }
            Self::list_insert(&mut self.by_role[r], id);
            if let Some(b) = employer {
                Self::list_insert(self.employers.entry(b).or_default(), id);
                self.employer_of.insert(id, b);
            }
        }
    }

    pub fn unindex_job(&mut self, id: EntityId) {
        self.mark_claims_job(id.index);
        for list in &mut self.by_role {
            Self::list_remove(list, id);
        }
        if let Some(b) = self.employer_of.remove(&id) {
            if let Some(list) = self.employers.get_mut(&b) {
                Self::list_remove(list, id);
                if list.is_empty() {
                    self.employers.remove(&b);
                }
            }
        }
    }

    /// J3: everyone employed at `building`, ascending (the employer index).
    pub fn staff_of(&self, building: EntityId) -> &[EntityId] {
        self.employers.get(&building).map_or(&[], Vec::as_slice)
    }

    /// Agents with a Brain at this LOD, ascending.
    pub fn tier(&self, lod: Lod) -> &[EntityId] {
        &self.by_tier[lod as usize]
    }

    /// Full and Coarse agents (the ones with bodies), ascending.
    pub fn bodies(&self) -> Vec<EntityId> {
        let (a, b) = (&self.by_tier[Lod::Full as usize], &self.by_tier[Lod::Coarse as usize]);
        let mut out = Vec::with_capacity(a.len() + b.len());
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            if a[i] < b[j] {
                out.push(a[i]);
                i += 1;
            } else {
                out.push(b[j]);
                j += 1;
            }
        }
        out.extend_from_slice(&a[i..]);
        out.extend_from_slice(&b[j..]);
        out
    }

    /// Scale S1: every agent with a Brain (the three tiers merged),
    /// ascending: `citizens()` with a Brain without walking every entity
    /// (buildings included) and their stores.
    pub fn brained(&self) -> Vec<EntityId> {
        let stat = &self.by_tier[Lod::Statistical as usize];
        let bodies = self.bodies();
        let mut out = Vec::with_capacity(stat.len() + bodies.len());
        let (mut i, mut j) = (0, 0);
        while i < bodies.len() && j < stat.len() {
            if bodies[i] < stat[j] {
                out.push(bodies[i]);
                i += 1;
            } else {
                out.push(stat[j]);
                j += 1;
            }
        }
        out.extend_from_slice(&bodies[i..]);
        out.extend_from_slice(&stat[j..]);
        out
    }

    /// Scale S4 perf: every building by door tile, built on the first query
    /// after a Building was inserted or removed (`building_gen`).
    pub fn building_grid(&self) -> std::sync::Arc<crate::spatial::BuildingGrid> {
        let mut memo = self.building_memo.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((g, grid)) = memo.as_ref() {
            if *g == self.building_gen {
                return grid.clone();
            }
        }
        let ids: Vec<EntityId> = self.with::<Building>();
        let doors: Vec<TilePos> =
            ids.iter().map(|&b| self.comp::<Building>(b).map_or_else(TilePos::default, |bd| bd.door)).collect();
        let grid = std::sync::Arc::new(crate::spatial::BuildingGrid::build(self.map.w(), self.map.h(), ids, &doors));
        *memo = Some((self.building_gen, grid.clone()));
        grid
    }

    /// Pressure pass round 5, U8 (OPEN-229): every Home a Purist gang could
    /// shake down for its chrome (`gang::gang_work_target`): a resident above
    /// the Purist tolerance (`creeds::chromed_home`), standing, not derelict,
    /// inhabited and held by no gang, by door, read on the first query of an
    /// hour. The pick searches it outward for the chromed first instead of
    /// reading every Home in the city per member think (21.7 % of the 100k
    /// profile at round 4's close). The pick re-tests every gate live, so a
    /// Home that left the list within the hour is never taken for one on it;
    /// one chromed or freed within the hour waits for the next (read per tick,
    /// the rebuild was 3 % of the 100k profile).
    pub fn chromed_homes(&self) -> std::sync::Arc<crate::spatial::BuildingGrid> {
        let _t = crate::prof::scope!("gang::chromed_homes");
        let hour = self.tick / time::TICKS_PER_HOUR;
        let mut memo = self.chromed_memo.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((h, grid)) = memo.as_ref() {
            if *h == hour {
                return grid.clone();
            }
        }
        let tol = self.config.creeds.creed_tolerance;
        let mut ids: Vec<EntityId> = self
            .kit
            .iter()
            .enumerate()
            .filter(|(_, k)| k.as_ref().is_some_and(|k| k.visible > tol))
            .filter_map(|(i, _)| {
                let id = EntityId { index: i as u32, generation: self.generations[i] };
                // `Household::home`, the one source of `residents_of`.
                self.comp::<Household>(id).and_then(|h| h.home)
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let mut held: Vec<EntityId> = self
            .gang_ids
            .iter()
            .filter_map(|&g| self.comp::<Gang>(g))
            .flat_map(|g| g.territory.iter().copied())
            .collect();
        held.sort_unstable();
        ids.retain(|&h| {
            held.binary_search(&h).is_err()
                && self.comp::<Building>(h).is_some_and(|b| {
                    b.kind == BuildingKind::Home && !b.demolished && !b.derelict && !b.occupants.is_empty()
                })
        });
        let doors: Vec<TilePos> =
            ids.iter().map(|&b| self.comp::<Building>(b).map_or_else(TilePos::default, |bd| bd.door)).collect();
        let grid = std::sync::Arc::new(crate::spatial::BuildingGrid::build(self.map.w(), self.map.h(), ids, &doors));
        *memo = Some((hour, grid.clone()));
        grid
    }

    /// Pressure pass round 5, U8 (OPEN-229): every guard's tile on a grid,
    /// read on the first query of a tick (`gang::guard_test`: it was built
    /// per GangWork target pick, two a member think). A guard who moved later
    /// in the tick is read where he stood at its first query.
    pub fn guard_grid(&self) -> std::sync::Arc<GuardGrid> {
        let mut memo = self.guard_memo.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((t, grid)) = memo.as_ref() {
            if *t == self.tick {
                return grid.clone();
            }
        }
        let tiles: Vec<TilePos> =
            self.guards().iter().filter_map(|&g| self.comp::<Position>(g).map(|p| p.tile)).collect();
        let grid = crate::spatial::TileGrid::build(self.map.w(), self.map.h(), 16, tiles.iter().copied());
        let grid = std::sync::Arc::new((tiles, grid));
        *memo = Some((self.tick, grid.clone()));
        grid
    }

    /// Scale S1: every live entity with a Position on a tile within the
    /// Chebyshev square of radius `r` around `tile`'s cell neighbourhood, or
    /// inside `building`, ascending, each once: a superset of "within `r`
    /// tiles, or inside `building`", for the caller's exact filter (a scan
    /// of every entity reads the same agents).
    pub fn agents_near(&mut self, tile: TilePos, r: u32, building: Option<EntityId>) -> Vec<EntityId> {
        let (w, h, n) = (self.map.w(), self.map.h(), self.alive.len());
        let (alive, position) = (&self.alive, &self.position);
        self.pos_index.refresh(w, h, n, |i| {
            let i = i as usize;
            if !alive[i] {
                return None;
            }
            position[i].as_ref().map(|p| (p.tile, p.building.map(|b| b.index)))
        });
        let mut idx = Vec::new();
        self.pos_index.near(tile, r, building.map(|b| b.index), &mut idx);
        idx.sort_unstable();
        idx.dedup();
        idx.into_iter()
            .filter(|&i| self.alive[i as usize])
            .map(|i| EntityId { index: i, generation: self.generations[i as usize] })
            .collect()
    }

    /// Job holders of one role, ascending.
    pub fn workers(&self, role: Role) -> &[EntityId] {
        self.by_role.get(role_index(role)).map_or(&[], Vec::as_slice)
    }

    /// J11: every role a Job can hold: the 19 bespoke roles (`Role::ALL`)
    /// and the configured trades, in [`World::workers`]' index order.
    pub fn roles(&self) -> Vec<Role> {
        let mut out = Role::ALL.to_vec();
        out.extend(self.config.trade_roles());
        out
    }

    /// Every guard (a Job holder with `Role::Guard`), ascending.
    pub fn guards(&self) -> &[EntityId] {
        self.workers(Role::Guard)
    }

    /// Rebuild both indices from the stores (after a load).
    pub fn rebuild_tiers_and_roles(&mut self) {
        let (tiers, roles, slots) = self.indices_from_stores();
        self.by_tier = tiers;
        self.by_role = roles;
        // The perf pass phase 5: `guards()` rebuilt wholesale; the claim
        // index with it, on its next query.
        *self.claims.0.get_mut().unwrap_or_else(|e| e.into_inner()) = None;
        self.stat_slots = slots;
        (self.employers, self.employer_of) = self.employers_from_stores();
        self.gang_ids = self.with::<Gang>();
        self.sentenced_ids = self.with::<Sentence>();
        self.corp_ids = self.with::<Corp>();
        (self.residents, self.resident_home) = self.residents_from_stores();
        self.squat_index = self.squat_index_from_stores();
        self.rebuild_reservation_index();
        self.rioter_of =
            self.riots.iter().flat_map(|r| r.rioters.iter().map(move |&a| (a, r.id))).collect::<BTreeMap<_, _>>();
        self.mean_price_cache = None;
    }

    fn squat_index_from_stores(&self) -> BTreeMap<EntityId, Vec<EntityId>> {
        let mut out: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        for id in self.entities() {
            if let Some(s) = self.comp::<Squatter>(id) {
                out.entry(s.building).or_default().push(id);
            }
        }
        out
    }

    /// J3: the employer index and its reverse from the Job store.
    #[allow(clippy::type_complexity)]
    fn employers_from_stores(&self) -> (BTreeMap<EntityId, Vec<EntityId>>, BTreeMap<EntityId, EntityId>) {
        let mut by_b: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        let mut back = BTreeMap::new();
        for id in self.entities() {
            if let Some(b) = self.comp::<Job>(id).and_then(|j| j.employer) {
                by_b.entry(b).or_default().push(id);
                back.insert(id, b);
            }
        }
        (by_b, back)
    }

    #[allow(clippy::type_complexity)]
    fn residents_from_stores(&self) -> (BTreeMap<EntityId, Vec<EntityId>>, BTreeMap<EntityId, EntityId>) {
        let mut residents: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        let mut back = BTreeMap::new();
        for id in self.entities() {
            if let Some(home) = self.comp::<Household>(id).and_then(|h| h.home) {
                residents.entry(home).or_default().push(id);
                back.insert(id, home);
            }
        }
        (residents, back)
    }

    fn indices_from_stores(&self) -> ([Vec<EntityId>; 3], Vec<Vec<EntityId>>, StatSlots) {
        let mut tiers: [Vec<EntityId>; 3] = Default::default();
        let mut roles: Vec<Vec<EntityId>> = vec![Vec::new(); Role::ALL.len() + self.config.trades.len()];
        let mut slots = StatSlots::default();
        for id in self.entities() {
            if let Some(b) = self.comp::<Brain>(id) {
                tiers[b.lod as usize].push(id);
                if b.lod == Lod::Statistical {
                    slots.0[StatSlots::slot_of(id)].push(id);
                }
            }
            if let Some(j) = self.comp::<Job>(id) {
                let r = role_index(j.role);
                if r >= roles.len() {
                    roles.resize(r + 1, Vec::new());
                }
                roles[r].push(id);
            }
        }
        (tiers, roles, slots)
    }

    /// Compare the incremental indices with a rebuild from the stores.
    pub fn check_indices(&self) -> Result<(), String> {
        let (tiers, roles, slots) = self.indices_from_stores();
        if slots != self.stat_slots {
            return Err("stat_slots out of sync with the Statistical tier".to_string());
        }
        if tiers != self.by_tier {
            return Err(format!(
                "by_tier out of sync: have {:?}, stores say {:?}",
                self.by_tier.iter().map(Vec::len).collect::<Vec<_>>(),
                tiers.iter().map(Vec::len).collect::<Vec<_>>()
            ));
        }
        if roles != self.by_role {
            return Err(format!(
                "by_role out of sync: have {:?}, stores say {:?}",
                self.by_role.iter().map(Vec::len).collect::<Vec<_>>(),
                roles.iter().map(Vec::len).collect::<Vec<_>>()
            ));
        }
        let (employers, employer_of) = self.employers_from_stores();
        if employers != self.employers || employer_of != self.employer_of {
            return Err(format!(
                "employers out of sync: have {} workplaces / {} workers, stores say {} / {}",
                self.employers.len(),
                self.employer_of.len(),
                employers.len(),
                employer_of.len()
            ));
        }
        let (residents, resident_home) = self.residents_from_stores();
        if residents != self.residents || resident_home != self.resident_home {
            return Err("residents out of sync with the Household stores".to_string());
        }
        if self.gang_ids != self.with::<Gang>() {
            return Err(format!("gang_ids out of sync: {:?}", self.gang_ids));
        }
        if self.sentenced_ids != self.with::<Sentence>() {
            return Err(format!("sentenced_ids out of sync: {:?}", self.sentenced_ids));
        }
        if self.corp_ids != self.with::<Corp>() {
            return Err(format!("corp_ids out of sync: {:?}", self.corp_ids));
        }
        if self.squat_index != self.squat_index_from_stores() {
            return Err("squat_index out of sync with the Squatter store".to_string());
        }
        if self.last_seen_by_tick != Self::last_seen_by_tick_of(&self.last_seen) {
            return Err("last_seen_by_tick out of sync with last_seen".to_string());
        }
        if let Some(idx) = self.report_index.get() {
            if *idx != Self::build_report_index(&self.crime_reports) {
                return Err("report_index out of sync with crime_reports".to_string());
            }
        }
        systems::motion::check(self)?;
        systems::assets::check(self)
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------
}
