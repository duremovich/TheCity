use super::*;

impl World {
    /// Add a memory, evicting the lowest `salience × recency` entry past the cap.
    pub fn remember(
        &mut self,
        id: EntityId,
        kind: MemoryKind,
        subject: Option<EntityId>,
        salience: f32,
        valence: f32,
        second_hand: bool,
    ) {
        let entry = MemoryEntry { subject, salience, valence, second_hand, ..MemoryEntry::blank(kind, self.tick) };
        self.remember_entry(id, entry);
    }

    /// `remember` with the entry built by the caller (the pressure pass R4:
    /// a fight's loser who started it carries `object` = himself, so the
    /// loss is no `Assaulted` deed on him; `memory::deed_of`).
    pub fn remember_entry(&mut self, id: EntityId, entry: MemoryEntry) {
        let kind = entry.kind;
        // Jobs & lives phase 5 (§ 5.2, D9): nothing is written under a chip.
        if !self.jobs2.chip.contracts.is_empty() && systems::dollhouse::under_chip(self, id) {
            return;
        }
        let tick = self.tick;
        let mut cap = self.config.brain.memory_cap;
        let half_life = self.config.brain.memory_half_life_days;
        // A Statistical agent keeps only what the hourly tick can act on, in
        // eight slots; the entries survive promotion, when the cap becomes 24.
        // M10 D30: the off-screen crimes and courtships the hourly table now
        // produces leave their memories too. Starved too: `needs::starvation`
        // drifts lawfulness once a day by checking for today's entry, and
        // without it a starving Statistical agent drifted every hour.
        if self.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical) {
            if !matches!(
                kind,
                MemoryKind::Grief
                    | MemoryKind::Starved
                    | MemoryKind::WasRobbed
                    | MemoryKind::MetInJail
                    | MemoryKind::Fought
                    | MemoryKind::Lost
                    | MemoryKind::Won
                    // L2 shadow fixes item 9: a Statistical fighter's bout.
                    | MemoryKind::Bout
                    | MemoryKind::Courted
                    | MemoryKind::Rejected
                    | MemoryKind::RentShort
                    | MemoryKind::Evicted
                    // Real economy E38: a parent's child taken (any tier).
                    | MemoryKind::ChildTaken
            ) {
                return;
            }
            cap = 8;
        }
        let second_hand = entry.second_hand;
        // M15 W15: a new deed memory may leave a grudge (never on a merge).
        let deed = systems::memory::deed_of(id, &entry);
        let Some(m) = self.comp_mut::<Memory>(id) else { return };
        let fresh = systems::memory::insert(m, entry, tick, cap, half_life);
        if let (true, Some(r)) = (fresh, deed) {
            systems::grudges::on_learn(self, id, &r, 1.0, u8::from(second_hand));
        }
    }

    /// The edge between two agents, if any.
    pub fn edge(&self, a: EntityId, b: EntityId) -> Option<&Edge> {
        self.edges.get(&edge_key(a, b))
    }

    /// The edge between two agents, created as an Acquaintance at affinity 0
    /// if absent. Keeps the neighbour index in step.
    pub fn edge_entry(&mut self, a: EntityId, b: EntityId) -> &mut Edge {
        let key = edge_key(a, b);
        let tick = self.tick;
        // One tree search when the edge exists (the usual case; perf).
        match self.edges.entry(key) {
            std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
            std::collections::hash_map::Entry::Vacant(v) => {
                self.edges_made += 1;
                self.neighbours.entry(a).or_default().insert(b);
                self.neighbours.entry(b).or_default().insert(a);
                v.insert(Edge::new(RelKind::Acquaintance, tick))
            }
        }
    }

    pub fn remove_edge(&mut self, a: EntityId, b: EntityId) {
        self.edges.remove(&edge_key(a, b));
        for (x, y) in [(a, b), (b, a)] {
            if let Some(s) = self.enemies.get_mut(&x) {
                s.remove(&y);
            }
        }
        if let Some(n) = self.neighbours.get_mut(&a) {
            n.remove(&b);
        }
        if let Some(n) = self.neighbours.get_mut(&b) {
            n.remove(&a);
        }
        for (x, y) in [(a, b), (b, a)] {
            if let Some(s) = self.kin_index.get_mut(&x) {
                s.remove(&y);
                if s.is_empty() {
                    self.kin_index.remove(&x);
                }
            }
        }
    }

    /// The perf pass phase 6: file the pair as kin candidates when its edge
    /// is a Spouse, Family or Parent edge now. Every write that makes an edge
    /// kin calls it after setting the kind.
    pub fn note_kin(&mut self, a: EntityId, b: EntityId) {
        let kin =
            self.edge(a, b).is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent));
        if kin {
            self.kin_index.entry(a).or_default().insert(b);
            self.kin_index.entry(b).or_default().insert(a);
        }
    }

    /// The perf pass phase 6 (the slope): `id`'s kin candidates, ascending:
    /// every neighbour whose edge is a Spouse, Family or Parent edge, and
    /// some whose edge was one. A walk of `neighbours(id)` that keeps only
    /// kin kinds reads the same agents in the same order from here (an
    /// agent's edges grow with the days; its kin do not).
    pub fn kin_candidates(&self, id: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.kin_index.get(&id).into_iter().flat_map(|s| s.iter().copied())
    }

    /// A first name for `sex` from the tables (seeded).
    pub fn random_first_name(&mut self, sex: Sex) -> String {
        let table = match sex {
            Sex::Male => &self.names.first_male,
            Sex::Female => &self.names.first_female,
        };
        if table.is_empty() {
            return "Nameless".to_string();
        }
        let i = self.rng.world().random_range(0..table.len());
        table[i].clone()
    }

    /// A full name from the tables (seeded).
    pub fn random_name(&mut self, sex: Sex) -> String {
        let first = self.random_first_name(sex);
        let last = if self.names.last.is_empty() {
            "Doe".to_string()
        } else {
            let i = self.rng.world().random_range(0..self.names.last.len());
            self.names.last[i].clone()
        };
        format!("{first} {last}")
    }

    /// After a load: the tables are not saved.
    /// The view milestone V1 (OPEN-262): every memo emptied after a config reload (each a pure function of the
    /// world and its config, built again on its next read).
    pub fn drop_memos(&mut self) {
        *self.located_memo.slot() = None;
        *self.building_memo.slot() = None;
        *self.chromed_memo.slot() = None;
        *self.guard_memo.slot() = None;
        *self.kind_memo.slot() = None;
        *self.shop_memo.slot() = None;
        *self.near_memo.slot() = None;
        *self.claims.slot() = None;
        *self.district_geo.slot() = None;
        self.mean_price_cache = None;
    }

    pub fn reload_names(&mut self) {
        self.names = NameTables::load(&self.config);
        let (table, legacy) = load_stat_table(&self.config);
        self.stat_table = table;
        self.stat_table_legacy = legacy;
        self.stat_mlp = load_stat_mlp(&self.config);
    }

    /// Set a `trace_flags` bit (ATE, SLEPT_AT_HOME) for today.
    pub fn mark_day(&mut self, id: EntityId, bit: u16) {
        *self.day_marks.entry(id).or_default() |= bit;
    }

    /// Remove an entity from every index and free it (emigrants, rotted or
    /// buried corpses). Edges to it are dropped.
    pub fn remove_agent(&mut self, id: EntityId) {
        if !self.is_alive(id) {
            return;
        }
        // M13 D20: a trip ends first, so the vehicle is parked, not lost.
        systems::vehicles::end_trip(self, id, false);
        systems::transit::end_ride(self, id);
        systems::motion::alight_now(self, id);
        // M16a (plan C16): an emigrant's records settle as a death's (a
        // refund lands in the wallet it leaves with).
        systems::contracts::on_death(self, id, None);
        // M11 D45: an emigrant's buildings pass to their heirs.
        systems::ownership::on_owner_gone(self, id);
        // M13 D13/D45: its parked vehicles too; an unsettled corpse settles
        // (the heirs are read from edges dropped below); what it carries goes.
        systems::assets::on_owner_gone(self, id);
        systems::assets::on_removed(self, id);
        self.abort_plan(id);
        self.vacate_job(id);
        self.remove_from_building(id);
        if self.has::<GangMember>(id) {
            systems::gang::leave(self, id, "gone");
        }
        crate::systems::social::unlink(self, id);
        for o in self.neighbours(id).collect::<Vec<_>>() {
            self.remove_edge(id, o);
        }
        self.neighbours.remove(&id);
        self.kin_index.remove(&id);
        self.release_all(id);
        self.reservations.retain(|_, rs| {
            rs.retain(|r| !matches!(r.kind, crate::exec::ReservationKind::Partner { other } | crate::exec::ReservationKind::Corpse { corpse: other } | crate::exec::ReservationKind::Suspect { suspect: other } if other == id));
            !rs.is_empty()
        });
        // The perf pass phase 4: every Partner reservation on `id` went.
        self.res_partners.remove(&id);
        self.pending_purchase.remove(&id);
        self.plan_queue.retain(|&(_, who), _| who != id);
        self.remove_last_seen(&id);
        self.vagrancy_places.remove(&id);
        // M12 D27/D20: a squatter's slot and a guest's bed are freed.
        self.remove::<Squatter>(id);
        self.hotel_beds.remove(&id);
        self.housing.forget(id);
        self.moving_in.remove(&id);
        self.sweep_beats.remove(&id);
        crate::systems::bind::drop_victim_holes(self, id);
        self.drop_reports_of(id);
        // Scale S1: the Brain holders, not every entity (only a Brain holds these).
        for id2 in self.brained() {
            if let Some(b) = self.comp_mut::<Brain>(id2) {
                if b.escorting == Some(id) {
                    b.escorting = None;
                }
                if b.cuffed_by == Some(id) {
                    b.cuffed_by = None;
                }
                if b.carrying_corpse == Some(id) {
                    b.carrying_corpse = None;
                }
            }
        }
        self.despawn(id);
    }

    /// After a load: every component store sized to the arena (a save leaves
    /// empty stores out, `skip_serializing_if = "all_none"`, and `#[serde(default)]`
    /// or `#[serde(skip)]` ones read back empty), and the litter grid to the map.
    pub fn resize_stores(&mut self) {
        self.resize_components();
        let tiles = self.map.w() * self.map.h();
        if self.litter.len() != tiles {
            self.litter.resize(tiles, 0);
        }
    }

    /// Rebuild `neighbours`, `spouses` and `enemies` from `edges` (after a
    /// load; the indices are not saved).
    pub fn rebuild_indices(&mut self) {
        self.plan_queue_age = self.plan_queue.iter().map(|(&(u, id), &t)| (t, u, id)).collect();
        // Hunger as sickness § 4: the stocked bins.
        let bins: BTreeSet<EntityId> = self
            .with::<Building>()
            .into_iter()
            .filter(|&b| self.comp::<Building>(b).is_some_and(|x| x.bin > 0))
            .collect();
        self.bins = bins;
        self.neighbours.clear();
        self.kin_index.clear();
        self.spouses.clear();
        self.enemies.clear();
        self.last_seen_by_tick = Self::last_seen_by_tick_of(&self.last_seen);
        let living = |w: &World, id: EntityId| w.has::<Identity>(id) && !w.has::<Corpse>(id);
        let edges: Vec<((EntityId, EntityId), RelKind)> = self.edges.iter().map(|(&k, e)| (k, e.kind)).collect();
        for ((a, b), kind) in edges {
            self.neighbours.entry(a).or_default().insert(b);
            self.neighbours.entry(b).or_default().insert(a);
            if matches!(kind, RelKind::Spouse | RelKind::Family | RelKind::Parent) {
                self.kin_index.entry(a).or_default().insert(b);
                self.kin_index.entry(b).or_default().insert(a);
            }
            match kind {
                RelKind::Spouse if living(self, a) && living(self, b) => {
                    self.spouses.insert(a, b);
                    self.spouses.insert(b, a);
                }
                // OPEN-112: as the spouses, the living only (`social::unlink`
                // drops the dead from the live index; the edge stays).
                RelKind::Enemy if living(self, a) && living(self, b) => {
                    self.enemies.entry(a).or_default().insert(b);
                    self.enemies.entry(b).or_default().insert(a);
                }
                _ => {}
            }
        }
        self.rebuild_tiers_and_roles();
        self.holes_by_agent.clear();
        for (&hid, h) in &self.holes {
            self.holes_by_agent.entry(h.victim).or_default().push(hid);
        }
        // M13 D2/D3: the asset indices, the corpses to settle, every Kit.
        systems::assets::rebuild(self);
        // M14 V1/V10: the plane's indices and the runners.
        systems::virt::rebuild_index(self);
        self.runner_of = self.runs.iter().map(|(&id, r)| (r.runner, id)).collect();
        // M15 W22/W35: the hunted and the guarded bodies.
        systems::hunt::reindex(self);
        systems::grudges::rebuild_guards(self);
        // L2 L15: who hangs out where.
        systems::leisure::rebuild_hangouts(self);
        // M16a (plan C2): the record indices.
        systems::contracts::rebuild_index(self);
        // The city milestone C6b: the tenancies by host, the vacant units.
        systems::floors::reindex(self);
    }

    /// Everyone `id` has an edge with, ascending.
    pub fn neighbours(&self, id: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.neighbours.get(&id).into_iter().flat_map(|s| s.iter().copied())
    }

    /// Marry two agents: a Spouse edge and the lookup in both directions.
    pub fn set_spouse(&mut self, a: EntityId, b: EntityId) {
        let tick = self.tick;
        let e = self.edge_entry(a, b);
        e.kind = RelKind::Spouse;
        e.last_interaction = tick;
        self.note_kin(a, b);
        self.spouses.insert(a, b);
        self.spouses.insert(b, a);
    }

    pub fn spouse_of(&self, id: EntityId) -> Option<EntityId> {
        self.spouses.get(&id).copied()
    }

    /// Everyone on an Enemy edge with `id`.
    pub fn enemies_of(&self, id: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.enemies.get(&id).into_iter().flat_map(|s| s.iter().copied())
    }

    /// A SawCrime memory that also records which crime was seen and (M15
    /// W5) on whom (`object`).
    pub fn remember_crime(
        &mut self,
        id: EntityId,
        subject: EntityId,
        crime: Crime,
        salience: f32,
        object: Option<EntityId>,
    ) {
        if !self.jobs2.chip.contracts.is_empty() && systems::dollhouse::under_chip(self, id) {
            return;
        }
        let tick = self.tick;
        let cap = self.config.brain.memory_cap;
        let half_life = self.config.brain.memory_half_life_days;
        let Some(m) = self.comp_mut::<Memory>(id) else { return };
        let entry = MemoryEntry {
            subject: Some(subject),
            salience,
            valence: -salience,
            crime: Some(crime),
            object,
            ..MemoryEntry::blank(MemoryKind::SawCrime, tick)
        };
        // M15 W15: a witness to a wrong done to someone close may hold a
        // grudge. A victim noticing its own wrong learns it once, through
        // its own `Lost` / `WasRobbed` (fix round: one deed, one grudge).
        let deed = object.filter(|&o| o != id).and_then(|_| systems::memory::deed_of(id, &entry));
        let fresh = systems::memory::insert(m, entry, tick, cap, half_life);
        if let (true, Some(r)) = (fresh, deed) {
            systems::grudges::on_learn(self, id, &r, 1.0, 0);
        }
    }

    /// Remove the Job, posting a vacancy at the employer. Returns the old Job.
    pub fn vacate_job(&mut self, id: EntityId) -> Option<Job> {
        // M15 W28: a departure's share of its group's competence.
        systems::competence::note_departure(self, id);
        let job = self.remove::<Job>(id)?;
        if let Some(employer) = job.employer {
            self.vacancies.entry(employer).or_default().push(job.role);
        }
        Some(job)
    }

    /// Drop the agent from its building's occupant list; the Position is left
    /// to the caller (a leaver moves to the street, a corpse stays put).
    /// M14 V12 (review): a seated runner taken off the list (put out at the
    /// door by a sack or a demolition, moved by an arrest) is out of the
    /// chair, so its run is dumped first; a no-op for anyone not on a run.
    pub fn remove_from_building(&mut self, id: EntityId) {
        if self.runner_of.contains_key(&id) {
            systems::virt::dump(self, id, "pulled from the chair");
        }
        let Some(here) = self.comp::<Position>(id).and_then(|p| p.building) else { return };
        if let Some(b) = self.comp_mut::<Building>(here) {
            if let Ok(i) = b.occupants.binary_search(&id) {
                b.occupants.remove(i);
            }
        }
    }

    /// Death: every component but `Position` and `Identity` goes, a `Corpse`
    /// is added, the building and household forget the agent, the job is
    /// vacated, and the event and daily counter are recorded.
    pub fn kill(&mut self, id: EntityId, cause: DeathCause) {
        self.kill_by(id, cause, None);
    }

    /// `kill` with the killer named, so a gang knows whether a rival did it.
    pub fn kill_by(&mut self, id: EntityId, cause: DeathCause, killer: Option<EntityId>) {
        if !self.has::<Identity>(id) || self.has::<Corpse>(id) {
            return;
        }
        // The pressure pass S6 (Dylan 2026-10-27, a hard rule): nobody under
        // 18 dies. Every pick skips them first; this is the backstop, counted
        // (`harm_blocked` above 0 is a pick site that missed).
        if systems::ages::is_protected(self, id) {
            systems::ownership::jobs2_row(self).harm_blocked += 1;
            return;
        }
        // Round 6 V3 (`OPEN-238`): the case's motive lead, read before the
        // dead's edges go: the killer and the dead knew each other.
        let tied = killer.is_some_and(|k| k != id && self.edge(k, id).is_some());
        // L2 phase 4 (plan L25): the ledger and the kill-rate tally read the
        // victim's tier, class and gang before anything is unlinked.
        systems::fviolence::note_death(self, id, cause, killer);
        // M16a (plan C16): the dead's records settle while its wallet is
        // still its own (a refund to a dead buyer is its estate's).
        systems::contracts::on_death(self, id, killer);
        // M14 V12: a body jacked in is dumped from its run first.
        systems::virt::dump(self, id, "died in the chair");
        let tick = self.tick;
        let name = self.name_of(id);
        let child = self.has::<Child>(id);
        // OPEN-282: every Death line prints the age and index (a starving
        // death's too), so a shadow can rule out a child.
        let age = systems::ages::years(self, id);
        // M13 D20 (phase 1 review): the trip ends before the heirs are dealt,
        // so a vehicle being driven is parked and inherited, not lost with
        // the corpse.
        systems::vehicles::end_trip(self, id, false);
        systems::transit::end_ride(self, id);
        systems::motion::alight_now(self, id);
        // C9a (measurement only): a leg ends at its walker's death (the log
        // held it open until the corpse went: C8b's "aborted Coarse walks"
        // of 2-3 days were the dead on their way, `elowen_17441 d22 10:45`).
        if self.leg_log.is_some() {
            crate::exec::leglog::end(self, id, "died");
        }
        // M13 D38: a dead dealer deals no more.
        systems::stims::end_deal(self, id);
        // L2 L15 (review fix): nor hangs out at a spot.
        systems::leisure::leave_spot(self, id);
        // M12 D17: a violent death leaves its mark on the street.
        if cause == DeathCause::Violence {
            if let Some((t, b)) = self.comp::<Position>(id).map(|p| (p.tile, p.building)) {
                systems::litter::deposit_near(self, t, b, 40, 1);
            }
        }
        // Widowhood is recorded on the Death event, and `on_death` unlinks the pair.
        let spouse = self.spouse_of(id);
        // A dead guard lets their suspect go; a dead suspect frees their guard.
        if let Some(b) = self.comp::<Brain>(id) {
            let (escorting, cuffed_by) = (b.escorting, b.cuffed_by);
            if let Some(s) = escorting {
                if let Some(sb) = self.comp_mut::<Brain>(s) {
                    sb.cuffed_by = None;
                }
            }
            if let Some(g) = cuffed_by {
                if let Some(gb) = self.comp_mut::<Brain>(g) {
                    gb.escorting = None;
                }
            }
        }
        // Witnesses, grief, widowhood and inheritance read the living state.
        // M13 D13: with a loot window the coins and goods stay on the body.
        let loot = systems::demography::on_death(self, id);
        // M15 W10: the kin are read before `on_death` unlinks the dead, and
        // the killing goes into the death's pool unnamed; a noticing
        // witness (`law::raise_crime_on`) or the binder names it.
        if cause == DeathCause::Violence {
            systems::gossip::post_killing(self, id);
        }
        // M15 W16: grudges on the dead settle, the dead's pass to its heirs
        // (read before `on_death` unlinks the spouse), its Hunts end.
        systems::grudges::on_death(self, id, killer);
        // M17 phase 5: the outside traces whoever killed one of its Outsiders.
        systems::abroad::on_death(self, id, killer);
        crate::systems::social::on_death(self, id);
        if cause == DeathCause::Violence && systems::law::is_guard(self, id) {
            systems::law_brain::push_shock(self, LawShock::GuardKilled);
        }
        // M11 D20: a corp loses an employee to violence.
        if cause == DeathCause::Violence {
            if let Some(corp) = self.comp::<Job>(id).and_then(|j| j.employer).and_then(|e| self.corp_of_building(e)) {
                systems::ownership::push_corp_shock(self, corp, CorpShock::EmployeeKilled);
            }
        }
        // M15 W28: a corp exec or the captain takes their competence share along.
        systems::competence::note_exec_death(self, id);
        self.vacate_job(id);
        self.remove_from_building(id);
        if self.has::<GangMember>(id) {
            systems::gang::on_member_killed(self, id, killer);
        }
        self.remove::<Needs>(id);
        self.remove::<Personality>(id);
        self.remove::<Mood>(id);
        self.remove::<Wallet>(id);
        self.remove::<Inventory>(id);
        self.remove::<Household>(id);
        self.remove::<Brain>(id);
        self.remove::<Memory>(id);
        self.remove::<Skills>(id);
        self.remove::<Sentence>(id);
        self.remove::<GangMember>(id);
        self.remove::<Child>(id);
        self.remove::<Squatter>(id);
        self.hotel_beds.remove(&id);
        self.housing.forget(id);
        self.moving_in.remove(&id);
        self.sweep_beats.remove(&id);
        self.release_all(id);
        self.pending_purchase.remove(&id);
        self.plan_queue.retain(|&(_, who), _| who != id);
        self.insert(
            id,
            Corpse {
                died_tick: tick,
                cause,
                buried: false,
                buried_tick: None,
                loot,
                stripped: false,
                settled: false,
                killer,
                tied,
                found: false,
                defended: false,
            },
        );
        systems::assets::on_corpse(self, id);
        match cause {
            DeathCause::Starvation => self.stats.current.deaths_starvation += 1,
            DeathCause::OldAge => self.stats.current.deaths_old_age += 1,
            DeathCause::Violence | DeathCause::Execution => self.stats.current.deaths_violence += 1,
            DeathCause::Accident => self.stats.current.crash_deaths += 1,
            DeathCause::Overdose => self.stats.current.overdoses += 1,
            // M14 V14: a Flatline is not violence.
            DeathCause::Flatline => self.stats.current.virt.flatlined += 1,
            // Hunger as sickness § 3.
            DeathCause::Disease => self.stats.current.health.deaths_disease += 1,
            // M17 review: a death abroad is the outside's count (`abroad_dead`).
            DeathCause::Abroad => {}
        }
        // `[dead, spouse?]`, or `[dead, spouse or NONE, killer]` when the
        // killer is known, so the biography names them (an attacker who
        // lost the fight has no Murder event saying who killed them).
        let actors: smallvec::SmallVec<[EntityId; 3]> = match (spouse, killer) {
            (s, Some(k)) => smallvec::smallvec![id, s.unwrap_or(EntityId::NONE), k],
            (Some(s), None) => smallvec::smallvec![id, s],
            (None, None) => smallvec::smallvec![id],
        };
        self.push_event(
            crate::events::EventKind::Death,
            &actors,
            format!("{name} #{} ({age} y) died of {cause:?}{}", id.index, if child { " (child)" } else { "" }),
        );
    }
}
