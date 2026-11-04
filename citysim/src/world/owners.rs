use super::*;

impl World {
    /// Every corp, ascending by id (an index kept by the Corp hooks).
    pub fn corps(&self) -> Vec<EntityId> {
        self.corp_ids.clone()
    }

    /// [`World::corps`] borrowed (no copy; a per-call read).
    pub fn corp_slice(&self) -> &[EntityId] {
        &self.corp_ids
    }

    /// Scale S4: the shipped row a corp copies (`[corps] template`): a
    /// seeded corp's row template, else its own name (a founded corp).
    pub fn corp_template(&self, corp: EntityId) -> &str {
        let Some(c) = self.comp::<Corp>(corp) else { return "" };
        match c.slot {
            Some(s) => self.config.corps.template_of(usize::from(s)),
            None => c.name.as_str(),
        }
    }

    /// Scale S4: the seeded corps copying the shipped row `template`, by
    /// row (the first copy first).
    pub fn corps_of_template(&self, template: &str) -> Vec<EntityId> {
        let mut v: Vec<(u16, EntityId)> = self
            .corp_ids
            .iter()
            .filter_map(|&c| self.comp::<Corp>(c).and_then(|cc| cc.slot).map(|s| (s, c)))
            .filter(|&(s, _)| self.config.corps.template_of(usize::from(s)) == template)
            .collect();
        v.sort_unstable();
        v.into_iter().map(|(_, c)| c).collect()
    }

    /// Review finding 7: how many copies of the opening roster the city
    /// was dealt (`Config::fit_to_map`'s `k`: the map's employers over the
    /// reference city's), read from the seeded corps (the most copies of any
    /// shipped row), at least 1. Every role-seeding pass (the Labs, the
    /// Feeds, the Garages and Clinics, the Fixers, the Unplugged) deals this
    /// many copies, so a corp row's copies and its buildings' agree (the
    /// population ratio, `Config::copies`, built 10 Labs for the one
    /// Zetatech of the shipped map at 20,000).
    pub fn seed_copies(&self) -> usize {
        let c = &self.config.corps;
        let templates: BTreeSet<&str> = (0..c.names.len()).map(|i| c.template_of(i)).collect();
        templates.into_iter().map(|t| self.corps_of_template(t).len()).max().unwrap_or(0).max(1)
    }

    /// Pressure pass F2: the copies of the roster whose opening businesses
    /// stand on Lots (`ScaleCfg::lot_copies`, at most [`World::seed_copies`]):
    /// every pass that builds on a Lot at `World::new` deals this many.
    pub fn lot_copies(&self) -> usize {
        let k = self.seed_copies();
        match self.config.scale.lot_copies as usize {
            0 => k,
            l => k.min(l),
        }
    }

    /// Position of a corp in `corps()`: picks its colour in the app.
    pub fn corp_index(&self, corp: EntityId) -> usize {
        self.corps().iter().position(|&c| c == corp).unwrap_or(0)
    }

    /// M12 D33: a riot closed this building and the closure has not run out.
    pub fn is_closed(&self, b: EntityId) -> bool {
        self.comp::<Building>(b).and_then(|bd| bd.closed_until).is_some_and(|t| t > self.tick)
    }

    /// A building's owner (`None` = the city).
    pub fn owner_of(&self, building: EntityId) -> Option<EntityId> {
        self.comp::<Building>(building).and_then(|b| b.owner)
    }

    /// The corp owning a building, if its owner is one.
    pub fn corp_of_building(&self, building: EntityId) -> Option<EntityId> {
        self.owner_of(building).filter(|&o| self.has::<Corp>(o))
    }

    /// The corp an agent works for: the one it is exec of, else the one
    /// owning its employer. A daily-or-rarer query (the exec lookup scans corps).
    pub fn corp_of_agent(&self, agent: EntityId) -> Option<EntityId> {
        self.corps()
            .into_iter()
            .find(|&c| self.comp::<Corp>(c).is_some_and(|cc| cc.exec == Some(agent)))
            .or_else(|| self.comp::<Job>(agent).and_then(|j| j.employer).and_then(|e| self.corp_of_building(e)))
    }

    /// M11 D13: where an agent collects wages: the Civic Hall for a city job
    /// (an employer the city owns, or none), else the employer building.
    pub fn wage_desk(&self, agent: EntityId) -> Option<EntityId> {
        let employer = self.comp::<Job>(agent).and_then(|j| j.employer);
        match employer {
            Some(e) if self.owner_of(e).is_some() => Some(e),
            _ => self.building_of_kind(BuildingKind::Hall),
        }
    }

    /// One Market's food price (`price_initial` if it has no `Market`): the
    /// rounded price fines, the fence, theft losses and the UI read.
    pub fn price_at(&self, market: EntityId) -> i64 {
        self.comp::<Market>(market).map_or(self.config.world.price_initial, |m| m.price_food)
    }

    /// A Market's price in tenths of a coin (`price_food x 10` when it
    /// carries no fraction).
    pub fn price_tenths_at(&self, market: EntityId) -> i64 {
        self.comp::<Market>(market).map_or(self.config.world.price_initial * 10, Market::tenths)
    }

    /// M11 phase 5: the whole-coin price `agent` pays at `market` today. A
    /// fractional price rounds up with probability equal to its fraction,
    /// from a hash of (agent, day, Market): no RNG draw, the same from
    /// planning to paying within a day, the right mean over many shoppers.
    /// Exactly `price_at` when the price is whole.
    pub fn price_for(&self, market: EntityId, agent: EntityId) -> i64 {
        let t = self.price_tenths_at(market);
        let (whole, frac) = (t / 10, t % 10);
        if frac == 0 {
            return whole.max(1);
        }
        let day = self.tick / crate::time::TICKS_PER_DAY;
        let mut h = (u64::from(agent.index) << 32) ^ day ^ (u64::from(market.index) << 48);
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        h ^= h >> 33;
        (whole + i64::from((h % 10) < frac as u64)).max(1)
    }

    /// The rounded mean price over every Market: the city-wide reading for
    /// wealth, stats, fines, Jail upkeep, the fence and the UI.
    pub fn mean_price(&self) -> i64 {
        // The perf pass phase 4: read once a `(shop_gen, kinds_gen)`.
        if crate::spatial::scans() {
            return self.mean_price_scan();
        }
        if let Some(p) = self.with_shop_memo(|m| m.mean_price) {
            return p;
        }
        let p = self.mean_price_scan();
        self.with_shop_memo(|m| m.mean_price = Some(p));
        p
    }

    fn mean_price_scan(&self) -> i64 {
        let prices: Vec<i64> = self
            .buildings_of_kind(BuildingKind::Market)
            .iter()
            .filter_map(|&m| self.comp::<Market>(m))
            .map(|m| m.price_food)
            .collect();
        if prices.is_empty() {
            return self.config.world.price_initial;
        }
        let n = prices.len() as i64;
        (prices.iter().sum::<i64>() + n / 2) / n
    }

    pub fn treasury(&self) -> Option<&Treasury> {
        self.building_of_kind(BuildingKind::Hall).and_then(|id| self.comp::<Treasury>(id))
    }

    pub fn treasury_mut(&mut self) -> Option<&mut Treasury> {
        self.building_of_kind(BuildingKind::Hall).and_then(|id| self.comp_mut::<Treasury>(id))
    }

    /// M9: the law, on the Jail.
    pub fn law(&self) -> Option<&Law> {
        self.building_of_kind(BuildingKind::Jail).and_then(|id| self.comp::<Law>(id))
    }

    pub fn law_mut(&mut self) -> Option<&mut Law> {
        self.building_of_kind(BuildingKind::Jail).and_then(|id| self.comp_mut::<Law>(id))
    }

    /// Every crime report, oldest first.
    pub fn crime_reports(&self) -> &[CrimeReport] {
        &self.crime_reports
    }

    /// The reports, for writing; drops the per-suspect index.
    pub fn reports_mut(&mut self) -> &mut Vec<CrimeReport> {
        self.report_index.take();
        self.seen_gen += 1;
        &mut self.crime_reports
    }

    /// Refresh the open report on `(suspect, crime)`, if any: its tick to
    /// now, its witness if it had none. Keeps a built index in step instead of
    /// dropping it (a rebuild walks every report; perf). `false` if none.
    pub fn refresh_open_report(
        &mut self,
        crime: crate::components::Crime,
        suspect: EntityId,
        witness: Option<EntityId>,
    ) -> bool {
        let tick = self.tick;
        let Some(r) = self.crime_reports.iter_mut().find(|r| !r.resolved && r.suspect == suspect && r.crime == crime)
        else {
            return false;
        };
        r.tick = tick;
        if r.witness.is_none() {
            r.witness = witness;
        }
        if let Some(idx) = self.report_index.get_mut() {
            if let Some(e) = idx.by_suspect.get_mut(&suspect) {
                e.1 = e.1.max(tick);
            }
        }
        true
    }

    /// Resolve every report on `suspect` (an arrest), keeping a built index
    /// in step (as `refresh_open_report`).
    pub fn resolve_reports_of(&mut self, suspect: EntityId) {
        self.seen_gen += 1;
        for r in self.crime_reports.iter_mut().filter(|r| r.suspect == suspect) {
            r.resolved = true;
        }
        if let Some(idx) = self.report_index.get_mut() {
            if let Some(e) = idx.by_suspect.get_mut(&suspect) {
                e.0 = 0;
            }
            if let Ok(i) = idx.open.binary_search(&suspect) {
                idx.open.remove(i);
            }
            idx.open_not_vagrancy.remove(&suspect);
        }
    }

    /// Drop every report on `suspect` (removed from the world), keeping a
    /// built index in step.
    pub fn drop_reports_of(&mut self, suspect: EntityId) {
        self.seen_gen += 1;
        self.crime_reports.retain(|r| r.suspect != suspect);
        if let Some(idx) = self.report_index.get_mut() {
            idx.by_suspect.remove(&suspect);
            if let Ok(i) = idx.open.binary_search(&suspect) {
                idx.open.remove(i);
            }
            idx.open_not_vagrancy.remove(&suspect);
        }
    }

    /// Append a report, keeping a built index in step (as `refresh_open_report`).
    pub fn push_report(&mut self, r: CrimeReport) {
        self.seen_gen += 1;
        if let Some(idx) = self.report_index.get_mut() {
            let e = idx.by_suspect.entry(r.suspect).or_insert((0, 0));
            e.0 += u32::from(!r.resolved);
            e.1 = e.1.max(r.tick);
            if !r.resolved && r.crime != crate::components::Crime::Vagrancy {
                *idx.open_not_vagrancy.entry(r.suspect).or_insert(0) += 1;
            }
            if e.0 > 0 {
                if let Err(i) = idx.open.binary_search(&r.suspect) {
                    idx.open.insert(i, r.suspect);
                }
            }
        }
        self.crime_reports.push(r);
    }

    pub(super) fn build_report_index(reports: &[CrimeReport]) -> ReportIndex {
        let mut m: BTreeMap<EntityId, (u32, Tick)> = BTreeMap::new();
        let mut open_not_vagrancy: BTreeMap<EntityId, u32> = BTreeMap::new();
        for r in reports {
            let e = m.entry(r.suspect).or_insert((0, 0));
            e.0 += u32::from(!r.resolved);
            e.1 = e.1.max(r.tick);
            if !r.resolved && r.crime != crate::components::Crime::Vagrancy {
                *open_not_vagrancy.entry(r.suspect).or_insert(0) += 1;
            }
        }
        // M11 phase 2: the suspects with an open report, kept apart so the
        // guards' warrant checks do not walk 60 days of closed ones.
        let open = m.iter().filter(|(_, &(open, _))| open > 0).map(|(&s, _)| s).collect();
        ReportIndex { by_suspect: m, open, open_not_vagrancy }
    }

    fn report_index(&self) -> &ReportIndex {
        self.report_index.get_or_init(|| Self::build_report_index(&self.crime_reports))
    }

    /// `(open reports, latest report tick)` on `suspect`, if any report names them.
    pub fn reports_on(&self, suspect: EntityId) -> Option<(u32, Tick)> {
        self.report_index().by_suspect.get(&suspect).copied()
    }

    /// Scale S1: does `suspect` have an open report, every one of them for
    /// Vagrancy? (`law::vagrant_at_work`'s test, from the index.)
    pub fn wanted_only_for_vagrancy(&self, suspect: EntityId) -> bool {
        let idx = self.report_index();
        idx.by_suspect.get(&suspect).is_some_and(|e| e.0 > 0) && !idx.open_not_vagrancy.contains_key(&suspect)
    }

    /// Where and when the law last saw each suspect.
    pub fn last_seen(&self) -> &BTreeMap<EntityId, (TilePos, Tick)> {
        &self.last_seen
    }

    /// Record a sighting of `suspect` (`(tile, tick)`), keeping the by-tick
    /// index in step.
    pub fn set_last_seen(&mut self, suspect: EntityId, seen: (TilePos, Tick)) {
        self.seen_gen += 1;
        if let Some((_, old)) = self.last_seen.insert(suspect, seen) {
            self.last_seen_by_tick.remove(&(old, suspect));
        }
        self.last_seen_by_tick.insert((seen.1, suspect));
    }

    /// Forget `suspect`'s sighting.
    pub fn remove_last_seen(&mut self, suspect: &EntityId) {
        self.seen_gen += 1;
        if let Some((_, old)) = self.last_seen.remove(suspect) {
            self.last_seen_by_tick.remove(&(old, *suspect));
        }
    }

    /// The perf pass phase 5: does a guard other than `guard` claim `s` (an
    /// escort, or an Arrest plan bound to them)? The same answer as
    /// `life::claimed_suspects(self, guard).contains(&s)`, from the claim
    /// index ([`ClaimIndex`]), re-filed at each read from its marks.
    pub fn claimed_by_other(&self, guard: EntityId, s: EntityId) -> bool {
        let mut memo = self.claims.0.lock().unwrap_or_else(|e| e.into_inner());
        let ix = memo.get_or_insert_with(|| ClaimIndex::build(self));
        ix.refresh(self);
        ix.claimed_by_other(guard, s)
    }

    /// The perf pass phase 5: a Brain written (`comp_mut`, `insert`,
    /// `remove`): a mark on the claim index when it files this guard.
    pub(crate) fn mark_claims_brain(&mut self, index: u32) {
        if let Some(ix) = self.claims.0.get_mut().unwrap_or_else(|e| e.into_inner()).as_mut() {
            ix.mark_brain(index);
        }
    }

    /// The perf pass phase 5: `guards()` may have changed at this entity.
    pub(super) fn mark_claims_job(&mut self, index: u32) {
        if let Some(ix) = self.claims.0.get_mut().unwrap_or_else(|e| e.into_inner()).as_mut() {
            ix.mark(index);
        }
    }

    /// Scale S1: every suspect last seen at `since` or later, in tick order
    /// (ties ascending by id).
    pub fn seen_since(&self, since: Tick) -> impl Iterator<Item = EntityId> + '_ {
        self.last_seen_by_tick.range((since, EntityId { index: 0, generation: 0 })..).map(|&(_, s)| s)
    }

    /// Scale S1: the open-warrant suspects last seen at `tick - window` or
    /// later, ascending, each with its `last_seen` entry (the only suspects
    /// the law's `located` can pass). Memoised until a sighting or a report
    /// changes or the clock moves.
    pub fn located_candidates(&self, window: Tick) -> std::sync::Arc<Located> {
        let key = (self.tick, window, self.seen_gen);
        let mut memo = self.located_memo.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((t, w, g, v)) = memo.as_ref() {
            if (*t, *w, *g) == key {
                debug_assert_eq!(v.list, self.located_candidates_fresh(window), "located_memo is stale");
                return v.clone();
            }
        }
        let v = std::sync::Arc::new(Located::new(self.located_candidates_fresh(window), self.map.w(), self.map.h()));
        *memo = Some((key.0, key.1, key.2, v.clone()));
        v
    }

    fn located_candidates_fresh(&self, window: Tick) -> Vec<(EntityId, (TilePos, Tick))> {
        let mut out: Vec<(EntityId, (TilePos, Tick))> = self
            .seen_since(self.tick.saturating_sub(window))
            .filter(|&s| self.reports_on(s).is_some_and(|(open, _)| open > 0))
            .filter_map(|s| self.last_seen.get(&s).map(|&seen| (s, seen)))
            .collect();
        out.sort_unstable_by_key(|&(s, _)| s);
        out
    }

    pub(super) fn last_seen_by_tick_of(last_seen: &BTreeMap<EntityId, (TilePos, Tick)>) -> BTreeSet<(Tick, EntityId)> {
        last_seen.iter().map(|(&s, &(_, t))| (t, s)).collect()
    }

    /// Suspects with an open report, ascending, each once.
    pub fn open_suspects(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.report_index().open.iter().copied()
    }

    /// Every gang, ascending by id (map order of their Hideouts).
    pub fn gangs(&self) -> Vec<EntityId> {
        self.gang_ids.clone()
    }

    /// The gang ids, ascending, borrowed (no clone for read-only scans).
    pub fn gang_list(&self) -> &[EntityId] {
        &self.gang_ids
    }

    /// The gang an agent belongs to.
    pub fn gang_of(&self, agent: EntityId) -> Option<EntityId> {
        self.comp::<GangMember>(agent).map(|g| g.gang).filter(|&g| self.has::<Gang>(g))
    }

    /// A gang's Hideout, if the building still exists.
    pub fn hideout_of(&self, gang: EntityId) -> Option<EntityId> {
        self.comp::<Gang>(gang).map(|g| g.hideout).filter(|&h| self.has::<Building>(h))
    }

    /// The other gang; with more than two, the one holding the most territory.
    /// Pressure pass round 6, V2 (TURF § 1): with `[turf]`, the neighbour (the
    /// gang holding the most cells bordering ours), so a rival is in reach;
    /// the old rule for a gang holding no cell or bordering nobody.
    pub fn rival_of(&self, gang: EntityId) -> Option<EntityId> {
        if let Some(r) = crate::systems::turf::rival(self, gang) {
            return Some(r);
        }
        self.gang_ids
            .iter()
            .copied()
            .filter(|&g| g != gang)
            .filter_map(|g| self.comp::<Gang>(g).map(|gg| (std::cmp::Reverse(gg.territory.len()), g)))
            .min()
            .map(|(_, g)| g)
    }

    /// Position of a gang in `gangs()`: picks its colour in the app.
    pub fn gang_index(&self, gang: EntityId) -> usize {
        self.gang_ids.iter().position(|&g| g == gang).unwrap_or(0)
    }

    /// The agent's name, a building's `Kind#index`, an asset's `"{label}
    /// T{tier}"` (M13 D1), or `#index` for anything else.
    pub fn name_of(&self, id: EntityId) -> String {
        match self.comp::<Identity>(id) {
            Some(i) => i.name.clone(),
            None => match self.comp::<Building>(id) {
                // M15 W36: a Feed goes by its name.
                Some(b) if b.feed.is_some() => b.feed.as_ref().map(|f| f.name.clone()).unwrap_or_default(),
                Some(b) => format!("{}#{}", b.kind.label(), id.index),
                None => match self.comp::<Asset>(id) {
                    Some(a) => format!("{} T{}", a.kind.label(), a.tier),
                    None => format!("#{}", id.index),
                },
            },
        }
    }

    pub fn tick_of_day(&self) -> u16 {
        time::tick_of_day(self.tick)
    }

    pub fn day(&self) -> u64 {
        time::day(self.tick)
    }

    pub fn phase(&self) -> time::DayPhase {
        time::phase(self.tick)
    }

    pub fn season(&self) -> time::Season {
        time::season(self.tick)
    }

    pub fn is_dark(&self) -> bool {
        time::is_dark(self.tick)
    }

    /// Called by the app every frame; queues a `SetView` command only when the
    /// rect differs from the last one queued (or applied, if none is pending),
    /// so a paused app does not flood the command log. Headless runs leave it `None`.
    pub fn set_view(&mut self, rect: Option<Rect>) {
        let pending = self.command_queue.iter().rev().find_map(|c| match c {
            PlayerCommand::SetView(r) => Some(*r),
            _ => None,
        });
        if pending.unwrap_or(self.view_rect) != rect {
            self.push_command(PlayerCommand::SetView(rect));
        }
    }

    // -----------------------------------------------------------------------
    // Tick
    // -----------------------------------------------------------------------
}
