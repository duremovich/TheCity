use super::*;

/// M16a (plan C28): the arresting guard interrogates a taker (or crew) of
/// a Hit or Beat fulfilled within `interrogate_days`, once per record per
/// arrest: an Intimidate move (`moves::resolve_with`, the move's own
/// `WordNs::Move` draw) with the bonus `0.3 × Law.competence +
/// interrogate_bias`, the stake the buyer's name. A win puts `Hired` (actor
/// the buyer, object the target, hops 1, conf 1) in the guard and, for a
/// brokered record, a second naming the Fixer's owner; the Fixer warms by
/// `heat_per_named`. `contract_cleared` counts a Hit's taker jailed within
/// `interrogate_days` of the killing (once per record).
pub(super) fn interrogate(world: &mut World, guard: EntityId, suspect: EntityId) {
    use crate::contract::{ContractKind, ContractStatus};
    let Some(ids) = world.by_party.get(&suspect).cloned() else { return };
    let now = world.tick;
    let window = Tick::from(world.config.law.interrogate_days) * TICKS_PER_DAY;
    for id in ids {
        let Some(c) = world.contracts.get(&id).cloned() else { continue };
        let took = c.taker == Some(suspect) || c.crew.contains(&suspect);
        let done = matches!(c.status, ContractStatus::Fulfilled | ContractStatus::Reneged);
        if !took || !done || !c.kind.violent() {
            continue;
        }
        let Some(closed) = c.closed else { continue };
        if now.saturating_sub(closed) > window || c.interrogated.contains(&now) {
            continue;
        }
        // Review fix: once per record (the first arrest inside the window
        // marks `interrogated` below).
        if c.kind == ContractKind::Hit && c.interrogated.is_empty() {
            world.stats.current.contract.contract_cleared += 1;
        }
        if let Some(x) = world.contracts.get_mut(&id) {
            x.interrogated.push(now);
        }
        world.stats.current.contract.interrogations += 1;
        let Some(buyer) = c.buyer.or(c.agent) else { continue };
        let competence = world.law().map_or(0.0, |l| l.competence);
        let bonus = 0.3 * competence + world.config.law.interrogate_bias;
        let m = crate::word::SocialMove {
            actor: guard,
            target: suspect,
            kind: crate::word::MoveKind::Intimidate,
            stake: crate::word::Stake::Info { about: buyer },
        };
        let out = crate::systems::moves::resolve_with(world, &m, bonus);
        if !out.success || out.refused {
            continue;
        }
        world.stats.current.contract.interrogations_won += 1;
        let target = c.target.id();
        crate::systems::contracts::hear_named(world, id, guard, buyer, target, 1);
        if let Some(f) = c.broker {
            if let Some(owner) = world.owner_of(f).filter(|&o| world.has::<crate::components::Identity>(o)) {
                crate::systems::contracts::hear_named(world, id, guard, owner, target, 1);
            }
            let by = world.config.law.heat_per_named;
            crate::systems::contracts::heat_add(world, f, by);
        }
    }
}

/// M16a (plan C29), at midnight: for each Hit or Beat fulfilled within
/// `accessory_days` whose placing agent is living, free and not yet
/// charged, a city guard or a witness of the strike (a `SawCrime` of the
/// taker or crew within an hour of the killing) holding `Hired` about it at
/// `conf ≥ accessory_conf` (`contracts::holds_hired`: the memory, never
/// `known_by`) files `Crime::Conspiracy` against the placing agent (the
/// lowest holder id). The law's own records (buyer `None`, `Origin::Law`:
/// the death squad) are never charged, and the record's own side (its
/// taker, crew and the broker's owner) never files. `Accessory` with the
/// days since the killing.
pub fn accessory_check(world: &mut World) {
    use crate::contract::{ContractKind, ContractStatus};
    if world.contracts.is_empty() {
        return;
    }
    let now = world.tick;
    let window = Tick::from(world.config.law.accessory_days) * TICKS_PER_DAY;
    let cands: Vec<crate::contract::Contract> = world
        .contracts
        .values()
        .filter(|c| c.kind.violent() && matches!(c.status, ContractStatus::Fulfilled | ContractStatus::Reneged))
        // Review fix: the law does not prosecute its own records (the death
        // squad: buyer `None`, `Origin::Law`).
        .filter(|c| c.buyer.is_some() && c.origin != crate::contract::Origin::Law)
        .filter(|c| !c.charged && c.closed.is_some_and(|t| now.saturating_sub(t) <= window))
        .filter(|c| {
            c.agent.is_some_and(|a| {
                living(world, a)
                    && !world.has::<Sentence>(a)
                    && world.comp::<Brain>(a).is_some_and(|b| b.cuffed_by.is_none())
            })
        })
        .cloned()
        .collect();
    if cands.is_empty() {
        return;
    }
    let guards: BTreeSet<EntityId> = crate::systems::law_brain::guards(world).into_iter().collect();
    let broker_owner: BTreeMap<crate::contract::ContractId, EntityId> =
        cands.iter().filter_map(|c| c.broker.and_then(|f| world.owner_of(f)).map(|o| (c.id, o))).collect();
    let mut found: Vec<(crate::contract::ContractId, EntityId)> = Vec::new();
    // scan-ok: daily, the memory holders, only while a fulfilled record waits.
    for holder in world.with::<crate::components::Memory>() {
        if found.len() == cands.len() {
            break;
        }
        let Some(m) = world.comp::<crate::components::Memory>(holder) else { continue };
        let guard = guards.contains(&holder);
        for c in &cands {
            if found.iter().any(|&(id, _)| id == c.id) || Some(holder) == c.agent {
                continue;
            }
            // Review fix: the record's own side (its taker, a squad's crew,
            // the broker's owner) never files against its buyer.
            if Some(holder) == c.taker || c.crew.contains(&holder) || broker_owner.get(&c.id) == Some(&holder) {
                continue;
            }
            let witness = !guard && {
                let closed = c.closed.unwrap_or(0);
                m.entries.iter().any(|e| {
                    e.kind == MemoryKind::SawCrime
                        && e.subject.is_some_and(|s| Some(s) == c.taker || c.crew.contains(&s))
                        && e.tick.abs_diff(closed) <= 60
                })
            };
            if (guard || witness) && crate::systems::contracts::holds_hired(world, holder, c) {
                found.push((c.id, holder));
            }
        }
    }
    found.sort_unstable();
    for (id, holder) in found {
        let Some(c) = world.contracts.get(&id).cloned() else { continue };
        let Some(agent) = c.agent else { continue };
        file_report(world, Crime::Conspiracy, agent, Some(holder));
        if let Some(x) = world.contracts.get_mut(&id) {
            x.charged = true;
        }
        world.stats.current.contract.accessory += 1;
        let days = now.saturating_sub(c.closed.unwrap_or(now)) / TICKS_PER_DAY;
        let what = if c.kind == ContractKind::Hit { "killing" } else { "beating" };
        let target = c.target.id();
        let text = format!(
            "{} charged with Conspiracy in the {what} of {} ({days} days after)",
            world.name_of(agent),
            world.name_of(target)
        );
        world.push_event(EventKind::Accessory, &[agent, target, holder], text);
    }
}

pub(super) fn resolve_reports(world: &mut World, suspect: EntityId) {
    world.resolve_reports_of(suspect);
}

/// Put an agent in the Jail with a `Sentence`; a job survives a sentence of
/// three days or less.
pub fn sentence(world: &mut World, who: EntityId, crime: Crime, until: Tick, jail: EntityId) {
    // M12 D15: a Vagrancy sentence is counted where the vagrant was swept
    // (`World::vagrancy_places`, else where they stand), before the Jail.
    // Any other crime leaves the entry for the Vagrancy report it belongs to.
    let place = (crime == Crime::Vagrancy).then(|| vagrancy_place(world, who));
    // The pressure pass phase 5a: the run in the cells begins (the post held
    // going in, who reported him).
    crate::systems::consequence::on_sentenced(world, who, crime);
    // Whoever was escorting them is done; a cuffed prisoner is a contradiction.
    // Every escort, not only `cuffed_by`: when two guards had cuffed the same
    // suspect, the first to reach the Jail cleared `cuffed_by`, the second kept
    // escorting, and `follow_guard` walked the new prisoner out of the Jail
    // (M10 phase 5b: prisoners with a Sentence outside the Jail).
    let escorts: Vec<EntityId> = world
        .guards()
        .iter()
        .copied()
        .chain(world.comp::<Brain>(who).and_then(|b| b.cuffed_by))
        .filter(|&g| world.comp::<Brain>(g).is_some_and(|b| b.escorting == Some(who)))
        .collect();
    for g in escorts {
        if let Some(gb) = world.comp_mut::<Brain>(g) {
            gb.escorting = None;
        }
    }
    if let Some(b) = world.comp_mut::<Brain>(who) {
        b.cuffed_by = None;
    }
    world.abort_plan(who);
    world.leave_building(who);
    world.enter_building(who, jail);
    world.insert(who, Sentence { until_tick: until, crime });
    // Round 8 X3 (`OPEN-247`): a sentence of `record_min_days` is a record.
    if until.saturating_sub(world.tick) >= u64::from(world.config.careers.record_min_days) * TICKS_PER_DAY {
        crate::systems::misery::note_record(world, who);
    }
    world.remember(who, MemoryKind::WasArrested, None, 0.7, -0.5, false);
    if let Some(p) = world.comp_mut::<Personality>(who) {
        p.drift(Drift::Arrested);
    }
    if until.saturating_sub(world.tick) > 3 * TICKS_PER_DAY {
        world.vacate_job(who);
        // M12 D27: a long sentence loses the squat (and tonight's bed).
        world.remove::<crate::components::Squatter>(who);
        world.hotel_beds.remove(&who);
    }
    if let Some(b) = world.comp_mut::<Brain>(who) {
        b.current_goal = None;
        b.lod = crate::components::Lod::Coarse;
    }
    world.retier(who);
    let name = world.name_of(who);
    let days = until.saturating_sub(world.tick).div_ceil(TICKS_PER_DAY);
    let charge = charge_label(world, who, crime);
    world.push_event(EventKind::Sentence, &[who], format!("{name} sentenced to {days} days for {charge}"));
    if let Some(place) = place {
        crate::systems::street::note_vagrancy(world, place);
        let text = format!("{name} jailed for the night for vagrancy in {}", world.district_name(place));
        world.push_event(EventKind::Vagrancy, &[who], text);
    }
    crate::systems::gang::on_member_arrested(world, who);
}

/// Where a Vagrancy report against `who` is counted: the district it was
/// swept in (`World::vagrancy_places`, consumed), else where it stands.
pub(super) fn vagrancy_place(world: &mut World, who: EntityId) -> crate::components::DistrictId {
    world.vagrancy_places.remove(&who).unwrap_or_else(|| {
        let tile = world.comp::<Position>(who).map(|p| p.tile).unwrap_or_default();
        world.district_of(tile)
    })
}

pub(super) fn release_at_jail_door(world: &mut World, who: EntityId, _why: &str) {
    // The escort ended inside the Jail (or at its door): step out onto the street.
    world.leave_building(who);
}

/// Release a prisoner: `full` means the sentence was served to the end.
pub fn release(world: &mut World, who: EntityId, full: bool) {
    // L2 (L21): a held prisoner gets its body back inside, then walks out.
    if crate::systems::lod::is_held(world, who) {
        crate::systems::lod::set_lod(world, who, crate::components::Lod::Coarse);
    }
    // Lives at work phase 7: a night served in a Station's cells ends there.
    let from = if world
        .comp::<Position>(who)
        .and_then(|p| p.building)
        .is_some_and(|b| crate::systems::stations::is_station(world, b))
    {
        BuildingKind::Station.label()
    } else {
        "Precinct"
    };
    let Some(_) = world.remove::<Sentence>(who) else { return };
    world.leave_building(who);
    world.remember(who, MemoryKind::WasArrested, None, 0.7, -0.5, false);
    if let Some(n) = world.comp_mut::<Needs>(who) {
        n.belonging = 0.2;
        n.safety = 0.5;
    }
    if full {
        if let Some(p) = world.comp_mut::<Personality>(who) {
            p.drift(Drift::ServedFullSentence);
        }
    }
    let name = world.name_of(who);
    world.push_event(EventKind::Release, &[who], format!("{name} released from the {from}"));
    // The pressure pass phase 5a: the life he comes out to.
    crate::systems::consequence::on_released(world, who);
    crate::systems::gang::on_member_released(world, who);
}

/// M9: a convict is broken out. The `Sentence` goes, they stand at the Jail
/// door hunted (`safety` 0.3) and glad of it (`Escaped`), and the warrant
/// **reopens** for the sentence's crime, so a re-arrest sentences them afresh.
pub fn escape(world: &mut World, who: EntityId) {
    // L2 (L21): a held prisoner broken out is promoted to Coarse first.
    if crate::systems::lod::is_held(world, who) {
        crate::systems::lod::set_lod(world, who, crate::components::Lod::Coarse);
    }
    let Some(s) = world.remove::<Sentence>(who) else { return };
    world.leave_building(who);
    world.remember(who, MemoryKind::Escaped, None, 0.7, 0.4, false);
    if let Some(n) = world.comp_mut::<Needs>(who) {
        n.safety = 0.3;
    }
    file_report(world, s.crime, who, None);
    crate::systems::gang::on_member_released(world, who);
}

/// M9: a gang stormed the Jail. `won`: convicts were freed, which is the
/// shock that garrisons the Jail; `beaten` guards lost their fight.
pub fn on_breakout(world: &mut World, _gang: EntityId, won: bool, beaten: usize) {
    for _ in 0..beaten {
        crate::systems::law_brain::push_shock(world, LawShock::GuardBeaten);
    }
    if won {
        let now = world.tick;
        if let Some(l) = world.law_mut() {
            l.last_breakout_tick = Some(now);
        }
        crate::systems::law_brain::push_shock(world, LawShock::Jailbreak);
    }
}

/// M9: is every guard holding the Jail (`Posture::Garrison`)?
pub fn garrisoned(world: &World) -> bool {
    world.law().is_some_and(|l| l.posture == Posture::Garrison)
}

/// M9: does this guard hold the Jail on the shift `shift_key`? Patrol: the
/// split in two by roster rank; Crackdown: one in three; Garrison: everyone.
/// A Crackdown with nobody to crack down on is a Patrol. The rank is the
/// guard's place among the guards by entity index, so a small roster never
/// leaves the Jail empty: when no rank matches the shift, the first guard holds it.
pub fn jail_duty(world: &World, guard: EntityId, shift_key: i64) -> bool {
    // L2 shadow fixes item 10: the duty is fixed for a shift as its key
    // comes due (it flipped from Patrol to the desk at 13:13 and the guard
    // walked 100 min back).
    if let Some((k, v)) = world.comp::<Job>(guard).and_then(|j| j.duty_fixed) {
        if k == shift_key {
            return v;
        }
    }
    jail_duty_now(world, guard, shift_key)
}

/// `jail_duty` as the roster and posture read now.
pub(super) fn jail_duty_now(world: &World, guard: EntityId, shift_key: i64) -> bool {
    // M11 D18: only the city's guards hold the Jail, Garrison included. The
    // roster is read off the role index without a copy: this runs per guard
    // per tick (`credit_guard_shifts`) and in every guard's think.
    use crate::systems::law_brain::is_city_guard;
    if !is_city_guard(world, guard) {
        return false;
    }
    let mut modulus: i64 = match world.law().map_or((Posture::Patrol, None), |l| (l.posture, l.target)) {
        (Posture::Garrison, _) => return true,
        (Posture::Crackdown, Some(_)) => 3,
        _ => 2,
    };
    // Lives at work phase 7 (§ 5.2): with Stations standing, each post's
    // roster is its own (a Station's desk is `station_desk_share` of its
    // guards a shift; the Precinct's guards rank among themselves).
    let post = crate::systems::stations::station_post(world, guard);
    let stations = !world.buildings_of_kind(BuildingKind::Station).is_empty();
    if post.is_some() {
        let share = world.config.law.station_desk_share.max(f32::EPSILON);
        modulus = ((1.0 / share).round() as i64).max(1);
    }
    let same_post = |g: EntityId| {
        is_city_guard(world, g) && (!stations || crate::systems::stations::station_post(world, g) == post)
    };
    let all = world.guards();
    let rank = all.iter().take_while(|&&g| g.index < guard.index).filter(|&&g| same_post(g)).count() as i64;
    let slot = shift_key.rem_euclid(modulus);
    if rank % modulus == slot {
        return true;
    }
    rank == 0 && (all.iter().filter(|&&g| same_post(g)).count() as i64) <= slot
}
