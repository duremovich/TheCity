use super::*;

impl World {
    /// A step failed: the goal is replanned once; a second consecutive
    /// failure cools it.
    pub fn fail_plan(&mut self, id: EntityId) {
        if let Some(goal) = self.comp::<Brain>(id).and_then(|b| b.plan_goal()) {
            self.goal_failed(id, goal);
        }
        self.abort_plan(id);
        // M18 P3: a directed contract is replanned once, as the goal is
        // (a second failure in a row stops it; the board's row goes on).
        let again = self.comp::<Brain>(id).is_some_and(|b| {
            b.player.as_ref().is_some_and(|p| p.contract.is_some())
                && matches!(b.last_plan_failure, Some((crate::components::GoalKind::Contract, _)))
        });
        if again {
            crate::systems::player::job_next(self, id);
        }
        crate::systems::player::plan_ended(self, id);
    }

    /// A goal that cannot be planned at all: nothing will change in a tick, so
    /// it cools straight away (the replan-once rule is for steps that fail
    /// in execution, where the world may have moved).
    pub fn cool_goal(&mut self, id: EntityId, goal: crate::components::GoalKind) {
        let until = self.tick + self.cool_ticks(id, goal);
        if let Some(b) = self.comp_mut::<Brain>(id) {
            // Round 4 T3 (OPEN-222): a longer cool the plan itself just set
            // (a held contract waiting out its mark's cells) stands.
            let until = b.cooldowns.get(&goal).map_or(until, |&t| t.max(until));
            b.cooldowns.insert(goal, until);
            b.last_plan_failure = None;
        }
        leglog::goal_plan(self, goal, 1);
        self.drop_cooled_visit(id, goal);
    }

    /// Round 3 S2 (OPEN-222): this cool's length, the streak counted: the
    /// `n`th cool of a goal in a row lasts `goal_cooldown_ticks x
    /// cool_backoff^(n-1)`, at most `cool_backoff_max_ticks`; a streak whose
    /// last cool ended `cool_backoff_reset_ticks` ago starts again. Needs
    /// and duties (`GoalKind::backs_off`) and `cool_backoff_max_ticks` 0 keep
    /// the flat cool.
    fn cool_ticks(&mut self, id: EntityId, goal: crate::components::GoalKind) -> Tick {
        let c = &self.config.brain;
        let (base, mult, max, reset) =
            (c.goal_cooldown_ticks, c.cool_backoff, c.cool_backoff_max_ticks, c.cool_backoff_reset_ticks);
        if max == 0 || !goal.backs_off() {
            return base;
        }
        let tick = self.tick;
        let Some(b) = self.comp_mut::<Brain>(id) else { return base };
        let n = b.cool_streak.get(&goal).filter(|&&(_, end)| tick <= end + reset).map_or(0, |&(n, _)| n);
        let ticks = ((base as f64) * f64::from(mult.max(1.0)).powi(i32::from(n))).min(max.max(base) as f64) as Tick;
        b.cool_streak.insert(goal, (n.saturating_add(1), tick + ticks));
        ticks
    }

    /// Shadow V5 bug 3: a Visit that cools (it could not plan, or its plan
    /// failed twice) is dropped for the day, not re-chosen when the cooldown
    /// lapses (each win had cut a shift). Nothing for any other goal.
    fn drop_cooled_visit(&mut self, id: EntityId, goal: crate::components::GoalKind) {
        if goal == crate::components::GoalKind::Visit {
            self.visits.remove(&id);
        }
    }

    /// A goal could not be planned or its plan failed: replanned once; a
    /// second consecutive failure of the same goal (no plan completed in
    /// between) cools it for `goal_cooldown_ticks`. The spec's "within 60
    /// ticks" window is shorter than one door wait plus think latency, so
    /// consecutive-ness is the usable reading.
    pub fn goal_failed(&mut self, id: EntityId, goal: crate::components::GoalKind) {
        let tick = self.tick;
        let second = self.comp::<Brain>(id).and_then(|b| b.last_plan_failure).is_some_and(|(g, _)| g == goal);
        let cooldown = if second { self.cool_ticks(id, goal) } else { 0 };
        if let Some(b) = self.comp_mut::<Brain>(id) {
            match b.last_plan_failure {
                Some((g, _)) if g == goal => {
                    b.cooldowns.insert(goal, tick + cooldown);
                    b.last_plan_failure = None;
                }
                _ => {
                    b.last_plan_failure = Some((goal, tick));
                    return;
                }
            }
        }
        leglog::goal_plan(self, goal, 1);
        self.drop_cooled_visit(id, goal);
    }

    /// Drop the current plan: abandon the running step (partial effects per
    /// `actions::on_abort`), release reservations, reset execution.
    pub fn abort_plan(&mut self, id: EntityId) {
        // Pressure pass F2: a Bury given up puts its body down where the
        // digger stands (it was toted into the next goal, then a Bury for
        // another corpse failed BuryCorpse with the first still carried, and
        // a second digger walked after the moving body: 337 and 404
        // `PreconditionLost` in 30 days at 20k).
        let burying = self.comp::<Brain>(id).is_some_and(|b| b.plan_goal() == Some(crate::components::GoalKind::Bury));
        if burying {
            if let Some(c) = self.comp::<Brain>(id).and_then(|b| b.carrying_corpse) {
                if self.has::<crate::components::Corpse>(c) {
                    if let Some(b) = self.comp_mut::<Brain>(id) {
                        b.carrying_corpse = None;
                    }
                }
            }
        }
        // A guard abandoning an escort lets the suspect go (to be re-arrested);
        // M13 D36: an abductor, its abductee.
        if let Some(suspect) = self.comp::<Brain>(id).and_then(|b| b.escorting) {
            if let Some(b) = self.comp_mut::<Brain>(suspect) {
                b.cuffed_by = None;
                b.abducted_by = None;
            }
            if let Some(b) = self.comp_mut::<Brain>(id) {
                b.escorting = None;
            }
        }
        let running = match self.comp::<Brain>(id).map(|b| &b.exec) {
            Some(ExecState::Use { kind, started, .. }) => Some((*kind, *started)),
            _ => None,
        };
        if let Some((kind, started)) = running {
            actions::on_abort(self, id, kind, started);
        }
        // M14 V12: a body pulled out of the chair dumps its run.
        if self.runner_of.contains_key(&id) {
            crate::systems::virt::dump(self, id, "pulled from the chair");
        }
        // M13 D20: a vehicle on the road parks where its driver stands.
        crate::systems::vehicles::end_trip(self, id, false);
        crate::systems::transit::end_ride(self, id);
        // The island pass phase 5 (I3): off the vehicle where it stands.
        crate::systems::motion::alight(self, id);
        // The island pass phase 9 (OPEN-181): a walk cut short leaves the
        // body where it stands on its way.
        settle_interrupted(self, id);
        self.segments.remove(&id);
        if !self.jobs2.riding.is_empty() {
            self.jobs2.riding.remove(&id);
        }
        if !self.jobs2.passengers.is_empty() {
            crate::systems::streetwork::passenger_off(self, id);
        }
        if self.leg_log.is_some() {
            leglog::end(self, id, "aborted");
        }
        self.commute_start.remove(&id);
        if let Some(b) = self.comp_mut::<Brain>(id) {
            b.clear_plan();
            b.shop_pick = None;
        }
        self.release_all(id);
    }
}
