use super::*;

impl World {
    /// Living citizens: entities with an `Identity` and no `Corpse`, ascending.
    pub fn citizens(&self) -> Vec<EntityId> {
        self.entities().filter(|&id| self.has::<Identity>(id) && !self.has::<Corpse>(id)).collect()
    }

    pub fn population(&self) -> usize {
        self.citizens().len()
    }

    /// The first building of a kind (the only one for singletons).
    pub fn building_of_kind(&self, kind: BuildingKind) -> Option<EntityId> {
        self.buildings_by_kind().get(&kind).and_then(|v| v.first().copied())
    }

    /// Every building of a kind, ascending (map order), demolished included.
    pub fn buildings_of_kind(&self, kind: BuildingKind) -> &[EntityId] {
        self.buildings_by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// Every kind's buildings (`buildings_of_kind`), read only.
    pub fn buildings_by_kind(&self) -> &BTreeMap<BuildingKind, Vec<EntityId>> {
        &self.buildings_by_kind
    }

    /// The perf pass phase 2: the kind lists for writing. Every write goes
    /// through here and bumps `kinds_gen`, which drops the venue index
    /// (`kind_grid`): its one invalidation point besides a Building inserted
    /// or removed (`building_gen`).
    pub fn buildings_by_kind_mut(&mut self) -> &mut BTreeMap<BuildingKind, Vec<EntityId>> {
        self.kinds_gen += 1;
        &mut self.buildings_by_kind
    }

    /// The perf pass phase 2 (the venue index): `buildings_of_kind(kind)`
    /// that have a Building, bucketed by door, built on the first query
    /// after the lists or the Building store changed (`kinds_gen`,
    /// `building_gen`; doors never move). A query is a superset the caller
    /// filters by its exact rule (`BuildingGrid::first_by_distance`).
    pub fn kind_grid(&self, kind: BuildingKind) -> std::sync::Arc<crate::spatial::BuildingGrid> {
        let key = (self.building_gen, self.kinds_gen);
        let mut memo = self.kind_memo.0.lock().unwrap_or_else(|e| e.into_inner());
        if memo.as_ref().is_some_and(|(k, _)| *k != key) {
            *memo = None;
        }
        let (_, grids) = memo.get_or_insert_with(|| (key, BTreeMap::new()));
        grids
            .entry(kind)
            .or_insert_with(|| {
                let (ids, doors): (Vec<EntityId>, Vec<TilePos>) = self
                    .buildings_of_kind(kind)
                    .iter()
                    .filter_map(|&b| self.comp::<Building>(b).map(|bd| (b, bd.door)))
                    .unzip();
                std::sync::Arc::new(crate::spatial::BuildingGrid::build(self.map.w(), self.map.h(), ids, &doors))
            })
            .clone()
    }

    /// The perf pass phase 4: [`crate::spatial::NearTable`] over
    /// `buildings_of_kind(kind)` that have a Building, built on the first
    /// query after the lists or the Building store changed (`kinds_gen`,
    /// `building_gen`), as [`World::kind_grid`] is.
    pub fn near_table(&self, kind: BuildingKind) -> std::sync::Arc<crate::spatial::NearTable> {
        let key = (self.building_gen, self.kinds_gen);
        let mut memo = self.near_memo.0.lock().unwrap_or_else(|e| e.into_inner());
        if memo.as_ref().is_some_and(|(k, _)| *k != key) {
            *memo = None;
        }
        let (_, tables) = memo.get_or_insert_with(|| (key, BTreeMap::new()));
        tables
            .entry(kind)
            .or_insert_with(|| {
                let _t = crate::prof::scope!("near_table::build");
                let (ids, doors): (Vec<EntityId>, Vec<TilePos>) = self
                    .buildings_of_kind(kind)
                    .iter()
                    .filter_map(|&b| self.comp::<Building>(b).map(|bd| (b, bd.door)))
                    .unzip();
                std::sync::Arc::new(crate::spatial::NearTable::build(self.map.w(), self.map.h(), ids, &doors))
            })
            .clone()
    }

    /// The view milestone V2b (OPEN-265): the Bars' and Markets' occupant
    /// lists' summed length and every occupant on them, ascending (Court's
    /// candidates at a venue, `social::candidate_at_venue_in`), read once a
    /// `(shop_gen, kinds_gen)`: an occupant list is written only through a
    /// Building write, which bumps `shop_gen`.
    pub fn venue_occupants(&self) -> (usize, std::sync::Arc<Vec<EntityId>>) {
        let key = (self.shop_gen, self.kinds_gen);
        let mut memo = self.venue_memo.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((k, v)) = memo.as_ref() {
            if *k == key {
                return v.clone();
            }
        }
        let mut list: Vec<EntityId> = [BuildingKind::Bar, BuildingKind::Market]
            .iter()
            .filter_map(|k| self.buildings_by_kind().get(k))
            .flatten()
            .filter_map(|&b| self.comp::<Building>(b))
            .flat_map(|bd| bd.occupants.iter().copied())
            .collect();
        let total = list.len();
        list.sort_unstable();
        list.dedup();
        let v = (total, std::sync::Arc::new(list));
        *memo = Some((key, v.clone()));
        v
    }

    /// The perf pass phase 4: `f` over `shop_memo` once it is current for
    /// `(shop_gen, kinds_gen)` (emptied first when it is not).
    pub(crate) fn with_shop_memo<R>(&self, f: impl FnOnce(&mut ShopMemo) -> R) -> R {
        let key = (self.shop_gen, self.kinds_gen);
        let mut memo = self.shop_memo.0.lock().unwrap_or_else(|e| e.into_inner());
        let m = memo.get_or_insert_with(ShopMemo::default);
        if m.key != key {
            *m = ShopMemo { key, ..ShopMemo::default() };
        }
        f(m)
    }

    /// The building of `kind` whose door is nearest `from` by Manhattan
    /// distance; ties to the lower index; demolished ones skipped.
    pub fn nearest_of_kind(&self, kind: BuildingKind, from: TilePos) -> Option<EntityId> {
        self.buildings_of_kind(kind)
            .iter()
            .filter_map(|&b| {
                self.comp::<Building>(b)
                    .filter(|bd| !bd.demolished && !bd.derelict)
                    .map(|bd| (bd.door.manhattan(from), b))
            })
            .min()
            .map(|(_, b)| b)
    }

    /// Workplace room: [`World::nearest_of_kind`] passing `except` over.
    fn nearest_of_kind_except(&self, kind: BuildingKind, from: TilePos, except: EntityId) -> Option<EntityId> {
        self.buildings_of_kind(kind)
            .iter()
            .filter(|&&b| b != except)
            .filter_map(|&b| {
                self.comp::<Building>(b)
                    .filter(|bd| !bd.demolished && !bd.derelict)
                    .map(|bd| (bd.door.manhattan(from), b))
            })
            .min()
            .map(|(_, b)| b)
    }

    /// The building of `kind` an agent uses: the one it is inside, else its
    /// employer if that is of `kind` (a clerk's own Market, a bartender's own
    /// Bar, as the Farm rule already did), else the nearest door to its tile
    /// (M10 D21). Stable between planning and execution because the agent
    /// stands on the same tile at both.
    pub fn local(&self, agent: EntityId, kind: BuildingKind) -> Option<EntityId> {
        let pos = self.comp::<Position>(agent)?;
        // M12 D33: a building a riot looted is closed for trade.
        let now = self.tick;
        let usable = |b: EntityId| {
            self.comp::<Building>(b).is_some_and(|bd| {
                bd.kind == kind && !bd.demolished && !bd.derelict && bd.closed_until.is_none_or(|t| t <= now)
            })
        };
        if let Some(b) = pos.building.filter(|&b| usable(b)) {
            return Some(b);
        }
        // Workplace room: the building that refused the agent at a full
        // door in the last `[exec] refused_ticks` is passed over (the
        // replan goes to the next one of the kind, or none: the home
        // pantry), not walked back to.
        let ttl = self.config.exec.refused_ticks;
        let refused = self
            .comp::<Brain>(agent)
            .and_then(|br| br.refused)
            .filter(|&(b, t)| ttl > 0 && now < t + ttl && self.comp::<Building>(b).is_some_and(|bd| bd.kind == kind))
            .map(|(b, _)| b);
        if let Some(e) = self.comp::<Job>(agent).and_then(|j| j.employer).filter(|&e| usable(e) && Some(e) != refused) {
            return Some(e);
        }
        let pick = if kind == BuildingKind::Market && self.config.corps.shop_price_tiles > 0 {
            self.market_by_price(pos.tile)
        } else {
            self.nearest_of_kind(kind, pos.tile)
        };
        match refused {
            Some(r) if pick == Some(r) => self.nearest_of_kind_except(kind, pos.tile, r),
            _ => pick,
        }
    }

    /// M11 D15: the Market a shopper at `from` picks, every tier alike: the
    /// least `door distance + shop_price_tiles × price` (ties lower id), so a
    /// cheaper Market draws custom from further off. Prices change only at
    /// midnight, so the choice holds from planning to execution. The price is
    /// read in tenths (phase 5), so a 2.7 beats a 3.0 by 1.2 tiles.
    /// Jobs and room P2 fix: an empty shelf is passed over while any Market
    /// holds food (seed 44's wages city starved 345 off-screen shoppers and
    /// thieves next to an empty Market for 37 days while the other two held
    /// 1,200 each); with every Market empty, the old pick.
    ///
    /// The perf pass phase 4: read off the Market [`World::near_table`]
    /// (geometry only; stock, closures and prices are read here, at query
    /// time). A price is never below 0 (`daily_price` floors it at a coin),
    /// so `10 x box distance` bounds a Market's cost from below: the bucket's
    /// list is read until that bound loses to the best stocked Market, and
    /// the scan runs only when the list runs out first (or `per_coin < 0`).
    pub fn market_by_price(&self, from: TilePos) -> Option<EntityId> {
        let per_coin = self.config.corps.shop_price_tiles;
        if per_coin >= 0 && !crate::spatial::scans() {
            let table = self.near_table(BuildingKind::Market);
            // A price is at least a coin (ten tenths), so the bucket's list
            // can settle the pick only when its box distance outruns a
            // coin's tiles (`per_coin`); at 252 tiles a coin it never did
            // on the 100k island (the view milestone V2b, 18M misses in two
            // days), and the list's Buildings were read for nothing.
            let rest = table.bucket(from).1;
            let found = (rest == u32::MAX || i64::from(rest) > per_coin)
                .then(|| {
                    table.min_cost(from, 10, |m, d| {
                        let b = self.comp::<Building>(m).filter(|b| !b.demolished && !self.is_closed(m))?;
                        Some((b.stock_food == 0, 10 * i64::from(d) + per_coin * self.price_tenths_at(m)))
                    })
                })
                .flatten();
            if let Some(m) = found {
                return m;
            }
            // The view milestone V2b (OPEN-265): since round 9 Y4 a coin is
            // worth 252 tiles, so the price term outruns any bucket's list and
            // every query fell to the scan, which read each Market's Building
            // (100k: 5 us a query, three or four a Statistical hour). The
            // cost from the table's door and the Market's price first; the
            // Building (closed, demolished, an empty shelf) only for a Market
            // whose `(cost, id)` beats the best stocked one so far: the scan's
            // pick whenever a stocked Market is open (none: the scan).
            let mut best: Option<(i64, EntityId)> = None;
            for (m, door) in table.entries() {
                let cost = 10 * i64::from(door.manhattan(from)) + per_coin * self.price_tenths_at(m);
                if best.is_some_and(|b| (cost, m) >= b) {
                    continue;
                }
                let stocked =
                    self.comp::<Building>(m).is_some_and(|b| !b.demolished && !self.is_closed(m) && b.stock_food > 0);
                if stocked {
                    best = Some((cost, m));
                }
            }
            if let Some((_, m)) = best {
                return Some(m);
            }
        }
        self.market_by_price_scan(from)
    }

    /// [`World::market_by_price`] by the scan of every Market (the
    /// reference, and the fallback).
    fn market_by_price_scan(&self, from: TilePos) -> Option<EntityId> {
        let per_coin = self.config.corps.shop_price_tiles;
        self.buildings_of_kind(BuildingKind::Market)
            .iter()
            .filter_map(|&m| {
                let b = self.comp::<Building>(m).filter(|b| !b.demolished && !self.is_closed(m))?;
                let cost = 10 * i64::from(b.door.manhattan(from)) + per_coin * self.price_tenths_at(m);
                Some((b.stock_food == 0, cost, m))
            })
            .min()
            .map(|(_, _, m)| m)
    }

    /// Jobs and room P2 fix: the Market `agent` eats from: [`World::local`]'s
    /// while it holds food, else the stocked one [`World::market_by_price`]
    /// picks from the agent's tile (a Clerk's own empty shop, the empty
    /// Market it stands in).
    pub fn food_market(&self, agent: EntityId) -> Option<EntityId> {
        let local = self.local(agent, BuildingKind::Market);
        if local.and_then(|m| self.comp::<Building>(m)).is_some_and(|b| b.stock_food > 0) {
            return local;
        }
        let tile = self.comp::<Position>(agent)?.tile;
        self.market_by_price(tile).filter(|&m| self.comp::<Building>(m).is_some_and(|b| b.stock_food > 0)).or(local)
    }

    // -----------------------------------------------------------------------
    // M11: corps and owners
    // -----------------------------------------------------------------------
}
