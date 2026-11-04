use super::*;

/// E39: each child eats `CHILD_FOOD_PER_DAY` from the camp's stock (the
/// Home's debt shape); the children the stock could not cover go unfed
/// (ascending id), `hunger_days` as at home (S6: the Reserve covers the
/// shortfall first; an unfed child never dies).
pub(super) fn feed(world: &mut World, camp: EntityId) {
    let kids = children_of(world, camp).to_vec();
    if kids.is_empty() {
        return;
    }
    let (needed, take) = {
        let Some(b) = world.comp_mut::<Building>(camp) else { return };
        b.child_food_debt += CHILD_FOOD_PER_DAY * kids.len() as f32;
        let needed = b.child_food_debt.floor() as u32;
        let take = needed.min(b.stock_food);
        b.stock_food -= take;
        b.child_food_debt -= take as f32;
        (needed, take)
    };
    // The pressure pass S6 (Dylan 2026-10-27, no child is harmed): camp
    // children are fed: a shortfall the camp's stock cannot meet is eaten
    // from the Reserve (the Warehouse), as the school meal; what neither
    // holds is an unfed day (the scandal's count), never a death.
    let short = needed - take;
    let reserve = world.building_of_kind(BuildingKind::Warehouse);
    let covered = reserve.and_then(|r| world.comp::<Building>(r)).map_or(0, |b| b.stock_food.min(short));
    if covered > 0 {
        if let Some(b) = reserve.and_then(|r| world.comp_mut::<Building>(r)) {
            b.stock_food -= covered;
        }
        if let Some(b) = world.comp_mut::<Building>(camp) {
            b.child_food_debt -= covered as f32;
        }
    }
    if let Some(s) = state_mut(world, camp) {
        s.reserve_today = covered;
    }
    let take = take + covered;
    let fed_n = if take == needed { kids.len() } else { (take as usize * 2).min(kids.len()) };
    for (i, &k) in kids.iter().enumerate() {
        let fed = i < fed_n;
        let Some(c) = world.comp_mut::<Child>(k) else { continue };
        if fed {
            c.hunger_days = 0;
        } else {
            c.hunger_days = c.hunger_days.saturating_add(1);
        }
        if let Some(s) = state_mut(world, camp) {
            if fed {
                if let Some(e) = s.fed_by_child.iter_mut().find(|(c, _)| *c == k) {
                    e.1 += 1;
                }
            } else {
                s.unfed_today = s.unfed_today.saturating_add(1);
            }
        }
    }
    let unfed = state(world, camp).map_or(0, |s| s.unfed_today);
    if let Some(s) = state_mut(world, camp) {
        s.fed_days.push_back(u8::from(unfed == 0));
        while s.fed_days.len() > RING_DAYS {
            s.fed_days.pop_front();
        }
        s.unfed_days.push_back(unfed);
        while s.unfed_days.len() > RING_DAYS {
            s.unfed_days.pop_front();
        }
    }
    world.stats.current.econ.camp_unfed += u32::from(unfed);
    if unfed > 0 {
        // Review fix: a child the service could not feed is a scandal
        // naming the service (S6: hungry, never starved to death).
        let what = world.name_of(camp);
        crate::systems::news::bulletin(
            world,
            &[camp],
            format!("the child protective service could not feed {unfed} children at {what}"),
        );
    }
}

/// E40: the shift is a daily ledger (children have no Brain): per child
/// older than `work_age`, `camp_yield × (floor + slope × farming)` Parts
/// into the camp's accumulator, whole Parts into its stock, `camp_skill_gain`
/// to the child's farming. Phase 2 (plan E13, E40): `input_per_part` to the
/// World per Part that enters the stock (`wages::produce_inputs`, wages on),
/// so a camp is no mint.
pub(super) fn shift(world: &mut World, camp: EntityId) {
    let cfg = world.config.camp.clone();
    let kids = children_of(world, camp).to_vec();
    let mut accum = 0.0f32;
    for k in kids {
        let years = world.comp::<Identity>(k).map_or(0, |i| i.age_years());
        if years < cfg.work_age {
            continue;
        }
        let farming = world.comp::<Skills>(k).map_or(0.0, |s| s.farming);
        accum += cfg.camp_yield * (cfg.camp_skill_floor + cfg.camp_skill_slope * farming);
        if let Some(s) = world.comp_mut::<Skills>(k) {
            s.farming = (s.farming + cfg.camp_skill_gain).min(1.0);
        }
    }
    if accum <= 0.0 {
        return;
    }
    let whole = {
        let Some(b) = world.comp_mut::<Building>(camp) else { return };
        b.production_accum += accum;
        let whole = b.production_accum.floor() as u32;
        b.production_accum -= whole as f32;
        whole
    };
    if whole == 0 {
        return;
    }
    let made = world.add_stock(camp, crate::components::Good::Parts, whole);
    if let Some(s) = state_mut(world, camp) {
        s.output_today += made;
    }
    world.stats.current.econ.camp_output += made;
    let per = world.config.economy2.input_per_part;
    crate::systems::wages::produce_inputs(world, camp, made, per);
}

/// E42: an unfed day adds a scandal day; the first is a Feed story and a
/// standing hit (read at the reputation rebuild); at `close_days` the law
/// closes the camp for `close_ban_days` and its children move.
pub(super) fn scandal(world: &mut World, camp: EntityId) {
    let unfed = state(world, camp).map_or(0, |s| s.unfed_today);
    if unfed == 0 {
        if let Some(s) = state_mut(world, camp) {
            if !s.closed_by_law {
                s.scandal_days = 0;
            }
        }
        return;
    }
    let days = {
        let Some(s) = state_mut(world, camp) else { return };
        s.scandal_days = s.scandal_days.saturating_add(1);
        s.scandal_days
    };
    world.stats.current.econ.camp_scandals += 1;
    let owner = world.owner_of(camp);
    let (what, who) = (world.name_of(camp), world.owner_label(owner));
    // Deviation: the third unfed day killed (as at home) before S6, so the
    // law closes the camp early: at `close_days − 1` unfed days (2 of the plan's
    // 3) the children move to the city's camp and eat there on the third.
    let close_days = world.config.camp.close_days.saturating_sub(1).max(1);
    if days == 1 || days == close_days {
        let mut actors = vec![camp];
        actors.extend(owner);
        crate::systems::news::bulletin(
            world,
            &actors,
            format!("{what} ({who}) could not feed {unfed} of its children"),
        );
    }
    if days >= close_days && state(world, camp).is_some_and(|s| !s.closed_by_law) {
        close(world, camp);
    }
}

/// E42: the law closes `camp`: `closed_by_law`, `closed_until`, its
/// children moved to the nearest other open camp (the city's last, over
/// capacity), `CampClosed`.
pub fn close(world: &mut World, camp: EntityId) {
    let ban = u64::from(world.config.camp.close_ban_days) * TICKS_PER_DAY;
    let now = world.tick;
    if let Some(s) = state_mut(world, camp) {
        s.closed_by_law = true;
    }
    if let Some(b) = world.comp_mut::<Building>(camp) {
        b.closed_until = Some(now + ban);
    }
    let owner = world.owner_of(camp);
    let kids = children_of(world, camp).to_vec();
    let from = door(world, camp).unwrap_or_default();
    let mut moved = 0;
    for k in kids {
        let Some((to, _)) = placement(world, from, None) else { break };
        if to == camp {
            break;
        }
        unhouse(world, k, camp);
        house(world, k, to);
        moved += 1;
    }
    let (what, who) = (world.name_of(camp), world.owner_label(owner));
    let mut actors = vec![camp];
    actors.extend(owner);
    world.push_event(EventKind::CampClosed, &actors, format!("the law closed {what} ({who}); {moved} children moved"));
}

/// E42: the owner's standing hit, `scandal_standing × unfed child-days in
/// the last 30 days` over its camps (negative; read at the reputation
/// rebuild: the store is rebuilt nightly, so a direct write would not last).
pub fn scandal_penalty(world: &World, owner: Option<EntityId>) -> f32 {
    let per = world.config.camp.scandal_standing;
    standing_camps(world)
        .into_iter()
        .filter(|&c| world.owner_of(c) == owner)
        .map(|c| state(world, c).map_or(0, |s| s.unfed_days.iter().map(|&u| u32::from(u)).sum::<u32>()))
        .sum::<u32>() as f32
        * per
}

// ---------------------------------------------------------------------------
// Release (E41)
// ---------------------------------------------------------------------------

/// E41 (`demography::mature`, after the adult's components are in): a
/// child whose home is a Camp is released: `true` when it was one.
pub fn on_mature(world: &mut World, id: EntityId) -> bool {
    let camp = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Camp));
    let Some(camp) = camp else { return false };
    release(world, id, ReleaseWhy::Adult);
    let _ = camp;
    true
}

/// E41, E42 (the Rescue hook): `id` leaves its camp: out of the lists,
/// homeless at the camp's door. `Adult`: the `CampRaised` trait, the
/// family's affinity, the loyalty or the grudge by the fed share, the
/// `CampRaised` event.
pub fn release(world: &mut World, id: EntityId, why: ReleaseWhy) {
    let Some(camp) = world
        .comp::<Household>(id)
        .and_then(|h| h.home)
        .filter(|&h| world.comp::<Building>(h).is_some_and(|b| b.kind == BuildingKind::Camp))
    else {
        return;
    };
    let (days, fed) = unhouse(world, id, camp);
    note_move(world, camp, id, why.camp_move());
    world.set_home(id, None);
    let now = world.tick;
    if let Some(h) = world.comp_mut::<Household>(id) {
        h.homeless_since = Some(now);
    }
    world.stand_at_door(id, camp);
    if why != ReleaseWhy::Adult {
        return;
    }
    let owner = world.owner_of(camp);
    let share = if days == 0 { 1.0 } else { (fed as f32 / days as f32).clamp(0.0, 1.0) };
    world.insert(id, CampRaised { camp, owner, days, fed_share: share });
    let cfg = world.config.camp.clone();
    let tick = world.tick;
    for p in parents_of(world, id) {
        let e = world.edge_entry(p, id);
        e.affinity = cfg.family_affinity;
        e.last_interaction = tick;
    }
    if share >= cfg.loyal_share {
        if let Some(pers) = world.comp_mut::<Personality>(id) {
            pers.loyalty = (pers.loyalty + 0.1).min(1.0);
        }
        if let Some(exec) = owner.and_then(|o| world.comp::<Corp>(o)).and_then(|c| c.exec) {
            let e = world.edge_entry(id, exec);
            e.affinity = (e.affinity + 0.2).clamp(-1.0, 1.0);
            e.last_interaction = tick;
        }
    } else if share < cfg.grudge_share {
        let target = owner
            .or_else(|| world.building_of_kind(BuildingKind::Jail).filter(|&j| world.has::<crate::components::Law>(j)));
        if let Some(t) = target {
            crate::systems::grudges::add(world, id, t, GrudgeCause::ChildTaken, 0.6, 0);
        }
    }
    let (name, what) = (world.name_of(id), world.name_of(camp));
    world.push_event(
        EventKind::CampRaised,
        &[id, camp],
        format!("{name} came of age at {what} after {days} days (fed {:.0} %)", share * 100.0),
    );
    world.stats.current.econ.camp_released += 1;
}

/// E41: a `CampRaised` applicant comes first for a Farm or Fab vacancy
/// (`demography::pick_candidate`'s key).
pub fn applicant_first(world: &World, id: EntityId, role: Role) -> bool {
    matches!(role, Role::Farmer | Role::Fabber) && world.has::<CampRaised>(id)
}
