use super::*;

/// The scripted steps' start check (in place of the planner's symbolic
/// preconditions): `AskAround` at the venue, `StakeOut` at the intel (a
/// Hunt) or beside the body (GuardBody), `Attack` within 4 tiles of a
/// living target.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind, target: Option<EntityId>) -> bool {
    let pos = world.comp::<Position>(id);
    let at_building = |b: EntityId| {
        pos.is_some_and(|p| {
            p.building == Some(b)
                || world
                    .comp::<Building>(b)
                    .is_some_and(|bd| crate::systems::law::chebyshev(p.tile, world.outside_door(bd)) <= 1)
        })
    };
    match kind {
        ActionKind::AskAround => {
            chase(world, id).and_then(|s| s.venue).is_some_and(at_building)
                && chase(world, id).is_some_and(|s| target_ok(world, s.target))
        }
        ActionKind::StakeOut => {
            let goal = world.comp::<Brain>(id).and_then(|b| b.plan_goal());
            if goal == Some(GoalKind::GuardBody) {
                return target.is_some_and(|c| {
                    world.comp::<crate::components::Corpse>(c).is_some_and(|k| !k.buried)
                        && crate::systems::law::near(world, id, c, 1)
                });
            }
            let Some(s) = chase(world, id) else { return false };
            if !target_ok(world, s.target) {
                return false;
            }
            match s.intel {
                Some(Intel { building: Some(b), .. }) => at_building(b),
                Some(i) => pos.is_some_and(|p| crate::systems::law::chebyshev(p.tile, i.tile) <= 1),
                None => false,
            }
        }
        // Pressure pass round 9, Y2 (`OPEN-250`): nor across the cells' wall
        // (the mark taken in while the hunter closed: V17's one blocked hit
        // at 100k reached the swing itself).
        ActionKind::Attack => target.is_some_and(|t| {
            crate::systems::law::living(world, t)
                && crate::systems::law::near(world, id, t, 4)
                && !crate::systems::law::held_apart(world, id, t)
        }),
        _ => crate::exec::actions::can_start(world, id, kind, target),
    }
}

/// W22: the stake-out begins (its timeout noted).
pub fn start_stakeout(world: &mut World, hunter: EntityId, until: Tick) {
    if let Some(mut s) = chase(world, hunter) {
        s.stakeout_until = Some(until);
        set_chase(world, hunter, s);
    }
    // Round 8 X2 (`OPEN-247`): a contract run at its mark's door is worked
    // (`contracts::stalled_runs` reads it).
    let now = world.tick;
    if let Some(r) = world.contract_runs.get_mut(&hunter) {
        r.worked = now;
    }
}

/// W22: the stake-out ran out with no contact: the Hunt goes back to
/// asking (intel cleared); a hunter sent the wrong way finds out
/// (`Deceived`, and a grudge on the liar at 0.3).
pub fn stakeout_failed(world: &mut World, hunter: EntityId) -> StepResult {
    let Some(s) = chase(world, hunter) else {
        return StepResult::Failed(crate::exec::FailReason::PreconditionLost);
    };
    if s.deceived {
        if let Some(liar) = s.liar {
            let text = format!("{} sent {} the wrong way", world.name_of(liar), world.name_of(hunter));
            world.push_event(EventKind::Deceived, &[hunter, liar], text);
            crate::systems::grudges::add(world, hunter, liar, GrudgeCause::Betrayed, 0.3, 0);
        }
    }
    if let Some(mut x) = chase(world, hunter) {
        x.phase = HuntPhase::Ask;
        x.intel = None;
        x.venue = None;
        x.deceived = false;
        x.liar = None;
        x.stakeout_until = None;
        set_chase(world, hunter, x);
    }
    reindex(world);
    StepResult::Failed(crate::exec::FailReason::PreconditionLost)
}

/// The kill chance of a strike: `Some(hunt_kill_p)` when `hunter` hunts
/// `victim` on a grudge of `lethal_min` or more.
pub fn strike_kill_p(world: &World, hunter: EntityId, victim: EntityId) -> Option<f32> {
    // M16a (plan C15): a contract Hit's taker strikes its target at
    // `hunt_kill_p` (a Beat at the base rate).
    if !world.contract_runs.is_empty() && !world.hunts.contains_key(&hunter) {
        return crate::systems::contracts::strike_kill_p(world, hunter, victim);
    }
    let s = world.hunts.get(&hunter).filter(|s| s.target == victim)?;
    let w = crate::systems::grudges::grudge_on(world, hunter, victim).map_or(s.weight, |g| g.weight);
    (w >= world.config.hunt.lethal_min).then_some(world.config.hunt.hunt_kill_p)
}

/// The respondent of an AskAround at `venue`: the co-occupant with the
/// highest affinity to the target (any edge; ties the lower id).
fn respondent(world: &World, hunter: EntityId, target: EntityId, venue: EntityId) -> Option<EntityId> {
    let b = world.comp::<Building>(venue)?;
    b.occupants
        .iter()
        .copied()
        .filter(|&o| o != hunter && o != target && crate::systems::law::living(world, o))
        .filter(|&o| crate::systems::demography::is_adult(world, o) && world.has::<Memory>(o))
        .filter_map(|o| world.edge(o, target).map(|e| (o, e.affinity)))
        .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
        .map(|(o, _)| o)
}

/// W21: `AskAround` completes. The respondent (else the Trace habit, no
/// move) trades one exchange each way with the hunter; a Friend of the
/// target may lie first (a Deceive against the hunter's knowledge: a
/// success sends the hunter to a random other Bar); else the hunter's move
/// (Charm on its own Friend, Intimidate when its dread is the higher,
/// Persuade otherwise, `+intel_k × knowledge` on its side): a success gives
/// the respondent's freshest sighting of the target, else the habit; a
/// failure leaves the target's Home. The Hunt moves to `Watch`.
pub fn ask_around(world: &mut World, hunter: EntityId) -> StepResult {
    let Some(s) = chase(world, hunter) else {
        return StepResult::Failed(crate::exec::FailReason::PreconditionLost);
    };
    let target = s.target;
    if !target_ok(world, target) {
        return StepResult::Failed(crate::exec::FailReason::PartnerLeft);
    }
    let now = world.tick;
    let venue = s.venue.or_else(|| world.comp::<Position>(hunter).and_then(|p| p.building));
    let r = venue.and_then(|v| respondent(world, hunter, target, v));
    // Round 9 Z2: a taker asking after his mark asks someone close to him,
    // who tells him.
    if let Some(r) = r {
        crate::systems::contracts::asked_after(world, hunter, r, venue);
    }
    let mut deceived_by = None;
    let intel = match r {
        None => habit(world, target, now),
        Some(r) => {
            crate::systems::gossip::exchange(world, r, hunter, crate::systems::gossip::Venue::Ask);
            crate::systems::gossip::exchange(world, hunter, r, crate::systems::gossip::Venue::Ask);
            let friend = world.edge(r, target).is_some_and(|e| e.kind == RelKind::Friend);
            let lie = friend && {
                let m = SocialMove {
                    actor: r,
                    target: hunter,
                    kind: MoveKind::Deceive,
                    stake: Stake::Info { about: target },
                };
                crate::systems::moves::resolve(world, &m).success
            };
            if lie {
                deceived_by = Some(r);
                wrong_bar(world, hunter, r, target)
            } else {
                let success = {
                    let kind = if world.edge(hunter, r).is_some_and(|e| e.kind == RelKind::Friend) {
                        MoveKind::Charm
                    } else if crate::systems::reputation::rep(world, hunter).dread
                        > crate::systems::reputation::rep(world, r).dread
                    {
                        MoveKind::Intimidate
                    } else {
                        MoveKind::Persuade
                    };
                    let bonus = world.config.hunt.intel_k * crate::systems::moves::knowledge(world, hunter);
                    let m = SocialMove { actor: hunter, target: r, kind, stake: Stake::Info { about: target } };
                    crate::systems::moves::resolve_with(world, &m, bonus).success
                };
                if success {
                    sightings_of(world, r, target)
                        .map(|(_, b, tile)| Intel { building: b, tile, source: IntelSource::Asked })
                        .unwrap_or_else(|| habit(world, target, now))
                } else {
                    world
                        .comp::<Household>(target)
                        .and_then(|h| h.home)
                        .and_then(|h| intel_at(world, h, IntelSource::Home))
                        .unwrap_or_else(|| habit(world, target, now))
                }
            }
        }
    };
    if let Some(mut x) = chase(world, hunter) {
        x.phase = HuntPhase::Watch;
        x.intel = Some(intel);
        x.stakeout_until = None;
        x.deceived = deceived_by.is_some();
        x.liar = deceived_by;
        set_chase(world, hunter, x);
    }
    reindex(world);
    StepResult::Done
}

/// A Friend's lie: a random Bar (Hunt stream) other than where the habit
/// puts the target.
fn wrong_bar(world: &World, hunter: EntityId, liar: EntityId, target: EntityId) -> Intel {
    let truth = habit(world, target, world.tick).building;
    let bars: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Bar)
        .iter()
        .copied()
        .filter(|&b| Some(b) != truth && world.comp::<Building>(b).is_some_and(|bd| !bd.demolished))
        .collect();
    if bars.is_empty() {
        return habit(world, target, world.tick);
    }
    let mut rng = world.rng.word(WordNs::Hunt, world.tick, (u64::from(hunter.index) << 32) | u64::from(liar.index));
    let b = bars[rng.random_range(0..bars.len())];
    intel_at(world, b, IntelSource::Deceived).unwrap_or_else(|| habit(world, target, world.tick))
}

/// W22, from `Attack`'s completion after its crime is raised: a hunter
/// striking its Hunt's target. A win posts the `Avenged` deed (and gives a
/// living target the first-hand memory), sets `kill_chain[target]` to the
/// grudge's chain + 1, settles the grudge (a death) or takes `beat_settle`
/// off it; a loss adds 0.1 and cools the Hunt `hunt_cooldown_days`. The
/// `HuntState` ends either way.
pub fn on_strike(world: &mut World, hunter: EntityId, target: EntityId, winner: EntityId, died: bool) {
    // M16a (plan C15): a contract taker's strike (no Hunt: returns below).
    crate::systems::contracts::on_strike(world, hunter, target, winner, died);
    let Some(s) = world.hunts.get(&hunter).filter(|s| s.target == target).cloned() else { return };
    world.hunts.remove(&hunter);
    reindex(world);
    let now = world.tick;
    let cfg = world.config.clone();
    // Settled already when the strike killed (by `grudges::on_death`).
    let grudge = world.comp::<Grudges>(hunter).and_then(|g| g.list.iter().rev().find(|x| x.target == target).cloned());
    if winner == hunter {
        world.kill_chain.insert(target, (s.chain.saturating_add(1), now));
        let tile = world
            .comp::<Position>(target)
            .or_else(|| world.comp::<Position>(hunter))
            .map_or_else(Default::default, |p| p.tile);
        let d = world.district_of(tile);
        crate::systems::gossip::post_deed(world, d, Deed::Avenged, Some(hunter), Some(target));
        if crate::systems::law::living(world, target) && world.has::<Memory>(target) {
            let sal = cfg.gossip.deed_sal.get(Deed::Avenged);
            let e = MemoryEntry {
                subject: Some(hunter),
                salience: sal,
                valence: -cfg.gossip.deed_sev.get(Deed::Avenged) * sal,
                deed: Some(Deed::Avenged),
                object: Some(target),
                ..MemoryEntry::blank(MemoryKind::Rumour, now)
            };
            crate::systems::memory::hear_entry(world, target, e);
        }
        let for_whom = for_whom(world, grudge.as_ref().map(|g| g.cause));
        let text = format!(
            "{} avenged {for_whom} on {}{}",
            world.name_of(hunter),
            world.name_of(target),
            if died { " (killed)" } else { "" }
        );
        world.push_event(EventKind::Avenged, &[hunter, target], text);
        if died {
            if let Some(g) = world.comp_mut::<Grudges>(hunter) {
                if let Some(x) = g.list.iter_mut().find(|x| x.target == target && x.settled.is_none()) {
                    x.settled = Some(now);
                }
            }
        } else {
            crate::systems::grudges::answer_beating(world, hunter, target);
            answered(world, hunter, target, grudge.as_ref().and_then(|g| killing(g.cause)));
        }
        let w = &mut world.stats.current.word;
        w.avenged += 1;
        w.hunts_ended += 1;
        if died {
            w.revenge_kills += 1;
        }
    } else {
        if let Some(g) = world.comp_mut::<Grudges>(hunter) {
            if let Some(x) = g.list.iter_mut().find(|x| x.target == target && x.settled.is_none()) {
                x.weight = (x.weight + 0.1).min(1.0);
            }
        }
        cool(world, hunter);
        world.stats.current.word.hunts_failed += 1;
        // Round 9 Z1: the end says how.
        if crate::systems::law::living(world, hunter) {
            let (he, _) = pro(world, hunter);
            let text = format!(
                "{} went for {}{} and lost; {he} nurses it, the grudge heavier",
                world.name_of(hunter),
                world.name_of(target),
                over(world, hunter, target)
            );
            ended(world, hunter, text);
        }
    }
}

/// Round 9 Z1: `by` beat `target` over `dead`: the others who hold a
/// grudge on him over the same dead take it as answered (`answer_beating`);
/// a Hunt of theirs on him whose grudge falls under its weight ends with
/// the line. A killing settles them all already (`grudges::on_death`).
pub(super) fn answered(world: &mut World, by: EntityId, target: EntityId, dead: Option<EntityId>) {
    let Some(dead) = dead else { return };
    // scan-ok: per Avenged beating, the grudge store.
    let others: Vec<EntityId> = world
        .with::<Grudges>()
        .into_iter()
        .filter(|&o| o != by)
        .filter(|&o| {
            world.comp::<Grudges>(o).is_some_and(|g| {
                g.list.iter().any(|x| x.target == target && x.settled.is_none() && killing(x.cause) == Some(dead))
            })
        })
        .collect();
    for o in others {
        crate::systems::grudges::answer_beating(world, o, target);
        let hunting = world.hunts.get(&o).is_some_and(|s| s.target == target);
        let w = crate::systems::grudges::grudge_on(world, o, target);
        if hunting && w.is_none_or(|g| g.settled.is_some() || g.weight < min_for(world, g.cause)) {
            end(world, o);
            reindex(world);
            let text = format!(
                "{} heard {} beat {} over {}: answered, {} let it go",
                world.name_of(o),
                world.name_of(by),
                world.name_of(target),
                for_whom(world, Some(GrudgeCause::KilledFriend(dead))),
                pro(world, o).0
            );
            ended(world, o, text);
        }
    }
}

/// Round 9 Z1: why a Hunt ends without a strike, in words (`None`: the
/// hunter is dead, nothing to say). A man held first is the wait's (the
/// grudge marked `NOTED_HELD`, `hunts_waiting`).
pub(super) fn end_words(world: &mut World, h: EntityId, t: EntityId) -> Option<String> {
    if !crate::systems::law::living(world, h) {
        return None;
    }
    let (hn, tn) = (world.name_of(h), world.name_of(t));
    let why = over(world, h, t);
    let (_, him) = pro(world, t);
    let (he, _) = pro(world, h);
    if !hunter_ok(world, h) {
        return Some(if held(world, h) {
            format!("{hn}'s hunt for {tn}{why} ended in {}", held_where(world, h))
        } else {
            format!("{hn} gave up on {tn}{why}: {he} is leaving")
        });
    }
    if !crate::systems::law::living(world, t) {
        return Some(format!("{tn} was dead before {hn} found {him}{why}"));
    }
    if held(world, t) {
        mark(world, h, t, NOTED_HELD);
        world.stats.current.word.hunts_waiting += 1;
        return Some(format!("the law took {tn} first ({}); {hn} will wait for {him}{why}", held_where(world, t)));
    }
    Some(format!("{tn} left the city before {hn} found {him}{why}"))
}

pub(super) fn cool(world: &mut World, hunter: EntityId) {
    let until = world.tick + Tick::from(world.config.hunt.hunt_cooldown_days) * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(hunter) {
        b.cooldowns.insert(GoalKind::Hunt, until);
    }
}

/// End `hunter`'s Hunt without a strike (its plan aborted if it is the Hunt's).
pub(super) fn end(world: &mut World, hunter: EntityId) {
    world.hunts.remove(&hunter);
    if world.comp::<Brain>(hunter).and_then(|b| b.plan_goal()) == Some(GoalKind::Hunt) {
        world.abort_plan(hunter);
    }
}

/// From `grudges::on_death`: the dead's own Hunt ends, and every Hunt on
/// the dead (its plan fails at the next step).
pub fn on_death(world: &mut World, dead: EntityId) {
    if world.hunts.is_empty() {
        return;
    }
    // A Hunt on the dead stays until `tick`: the strike that killed it
    // still reads it (`on_strike` runs after the fight's death).
    world.hunts.remove(&dead);
    reindex(world);
}
