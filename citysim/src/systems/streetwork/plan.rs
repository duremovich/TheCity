use super::*;

// ---------------------------------------------------------------------------
// Tinkers (§ 3)
// ---------------------------------------------------------------------------

/// `assets::repairs`: an agent's asset at `from` due a repair costing the
/// Garage `cost`: the nearest working tinker within `tinker_reach` with
/// jobs left fixes it at `tinker_discount` × `cost` (`Flow::Asset` to the
/// tinker), or wrecks it on `tinker_fail_p` (`TinkerBotched`). `Some(true)`
/// fixed, `Some(false)` wrecked, `None` no tinker (the Garage's path).
pub fn tinker_repair(world: &mut World, asset: EntityId, owner: EntityId, from: TilePos, cost: i64) -> Option<bool> {
    if !on(world) || world.has::<Corp>(owner) || !world.has::<Wallet>(owner) {
        return None;
    }
    let cfg = world.config.jobs2.clone();
    let tinker = world
        .jobs2
        .pitches
        .values()
        .filter(|p| p.owner != owner && p.jobs_today < cfg.tinker_jobs && p.spot.manhattan(from) <= cfg.tinker_reach)
        .filter(|p| kind_of_pitch(world, p) == Some(PitchKind::Tinker) && working(world, p))
        .min_by_key(|p| (p.spot.manhattan(from), p.owner))
        .map(|p| p.owner)?;
    let price = ((cost as f32 * cfg.tinker_discount).round() as i64).max(1);
    if coins(world, owner) < price {
        return None;
    }
    let paid = ownership::pay(world, Some(owner), Some(tinker), price, Flow::Asset);
    if let Some(p) = world.jobs2.pitches.get_mut(&tinker) {
        p.jobs_today = p.jobs_today.saturating_add(1);
        p.take_today += paid;
    }
    let row = ownership::jobs2_row(world);
    row.tinker_repairs += 1;
    row.tinker_take += paid;
    // Lives at work phase 4b (§ 2.3): the tinker's regulars (the pitch is a tile).
    if crate::systems::service::street_on(world) && world.has::<crate::components::Brain>(owner) {
        if let Some(d) = world.jobs2.pitches.get(&tinker).map(|p| p.district) {
            let kind = crate::components::ServeKind::Buyer;
            crate::systems::service::encounter(world, tinker, owner, None, d, kind, paid);
        }
    }
    let botched = hash(world, PURPOSE_TINKER, u64::from(asset.index), world.day()) < cfg.tinker_fail_p;
    if botched {
        ownership::jobs2_row(world).tinker_botched += 1;
        let (who, whose) = (world.name_of(tinker), world.name_of(owner));
        world.push_event(
            EventKind::TinkerBotched,
            &[tinker, owner],
            format!("{who} botched a street repair on {whose}'s vehicle (wrecked)"),
        );
        // Phase-8 review: the vehicle is wrecked as any wreck is (its Parts,
        // the event, the despawn); setting its condition to 0 left a zombie
        // vehicle no repair pass touches again.
        if world.has::<crate::components::Asset>(asset) {
            crate::systems::assets::wreck(world, asset, "a botched street repair");
        }
    }
    Some(!botched)
}

// ---------------------------------------------------------------------------
// GoalKind::Pitch (bodies)
// ---------------------------------------------------------------------------

/// The tile a body's Pitch goal walks to: its own Pitch's spot, else (in
/// the dawn window) the nearest corner within `corner_reach`.
pub fn street_tile(world: &World, id: EntityId) -> Option<TilePos> {
    if let Some(p) = world.jobs2.pitches.get(&id) {
        return Some(p.spot);
    }
    corner_for(world, id)
        .or_else(|| foreman_gate(world, id))
        // Round 4 T5 (OPEN-217): a searcher's door to ask at.
        .or_else(|| crate::systems::search::board_tile(world, id))
}

/// Round 4 T4 (OPEN-217; `[verbs] shape_up_gate`): in the dawn window after
/// a dawn that offered posts, a Yard's foreman with a body (a Construction
/// or Freight Yard's, `Job.foreman`, well and free) goes to its gate (the
/// outside door) for the shape-up: the gate's tile, else `None`. V12's
/// foreman picked 19-32 hands a morning from his bed.
pub fn foreman_gate(world: &World, id: EntityId) -> Option<TilePos> {
    if !world.config.verbs.as_ref().is_some_and(|v| v.shape_up_gate) || !dawn(world) || world.jobs2.last_posts == 0 {
        return None;
    }
    let j = world.comp::<Job>(id).filter(|j| j.foreman)?;
    let y = j.employer?;
    let bd = world.comp::<Building>(y)?;
    if !matches!(bd.kind, BuildingKind::ConstructionYard | BuildingKind::FreightYard)
        || !crate::systems::industry::open(world, y)
        || world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical)
        || world.has::<Sentence>(id)
        || crate::systems::health::sick(world, id)
        || !crate::exec::routine::is_workday(world.day() as i64)
    {
        return None;
    }
    // A Yard whose owner cannot pay a hand a day posts nothing (`day_posts`).
    let wage = day_role(world).map_or(0, |r| world.config.wage(r));
    if world.purse(world.owner_of(y)) < wage {
        return None;
    }
    Some(world.outside_door(bd))
}

/// Round 4 T4: the foreman's walk to the gate is due: `foreman_gate`, and
/// he stands there already or it is time to set out (the travel from where
/// he is and `[verbs] shape_up_lead_min` before `corner_hour`). The Pitch
/// goal's gate and the wake from a committed Sleep (`exec::actions`).
pub fn foreman_due(world: &World, id: EntityId) -> bool {
    let Some(gate) = foreman_gate(world, id) else { return false };
    let Some(p) = world.comp::<Position>(id) else { return false };
    if p.building.is_none() && p.tile.manhattan(gate) <= world.config.jobs2.corner_tiles {
        return true;
    }
    // On his way already: the walk holds (the travel read mid-route said
    // "too far" and he turned to Idle in the street).
    if world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(GoalKind::Pitch) {
        return true;
    }
    let lead = world.config.verbs.as_ref().map_or(0, |v| Tick::from(v.shape_up_lead_min));
    let (tod, pick) = (Tick::from(world.tick_of_day()), Tick::from(world.config.jobs2.corner_hour));
    let t = crate::systems::distance::travel(world, p.tile, gate);
    // Too far to make the pick from here: he does not set out.
    tod + t <= pick && tod + t + lead >= pick
}

/// Round 4 T4: the foreman `f` stands at `y`'s gate now (a body within
/// `[jobs2] corner_tiles` of its outside door, on the street; an
/// off-screen foreman counts as there).
pub(super) fn at_gate(world: &World, f: EntityId, y: EntityId) -> bool {
    if world.comp::<Brain>(f).is_some_and(|b| b.lod == Lod::Statistical) {
        return true;
    }
    let Some(bd) = world.comp::<Building>(y) else { return false };
    let gate = world.outside_door(bd);
    world
        .comp::<Position>(f)
        .is_some_and(|p| p.building.is_none() && p.tile.manhattan(gate) <= world.config.jobs2.corner_tiles)
}

/// The dawn window: the two hours before `corner_hour`.
pub(super) fn dawn(world: &World) -> bool {
    let h = world.config.jobs2.corner_hour;
    let tod = world.tick_of_day();
    tod < h && tod + 2 * TICKS_PER_HOUR as u16 >= h
}

fn corner_for(world: &World, id: EntityId) -> Option<TilePos> {
    if !dawn(world) || !corner_ok(world, id) {
        return None;
    }
    // The parity diagnosis: only after a dawn that offered posts (and
    // `corner_reach` is short, 40 tiles). With 100 tiles every broke jobless
    // body walked across the Vats before dawn, most were sent away, and the
    // all-Full parity city's hunger-days rose 1.06 -> 1.15 (a `p_corner`
    // roll on the walk fixed parity but crowded the Spire's HangOuts and
    // cost the CEOs their known contacts in the behaviour tier).
    if world.jobs2.last_posts == 0 {
        return None;
    }
    let from = world.comp::<Position>(id).map(|p| p.tile)?;
    let cs = corners(world);
    // The pressure pass phase 2: the jobless walk farther to a shape-up.
    let reach = crate::systems::customers::cfg(world).map_or(world.config.jobs2.corner_reach, |c| c.shape_up_reach);
    corner_near(&cs, from, reach).map(|i| cs[i].1)
}

/// The Pitch goal's considerations: one's own Pitch in its hours (as Work
/// scores a shift), or the broke jobless' dawn walk to the corner (as Earn
/// scores the wallet). `None` off, or with neither.
pub fn considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if !on(world) || world.has::<Sentence>(id) {
        return None;
    }
    // Round 4 T4 (OPEN-217): a Yard's foreman goes to the gate at dawn.
    if world.has::<Job>(id) {
        if !foreman_due(world, id) {
            return None;
        }
        let n = world.comp::<crate::components::Needs>(id)?;
        return Some((
            vec![
                Consideration::new("the shape-up at dawn", 1.0, GATE),
                Consideration::new("hunger", n.hunger, IDENTITY),
                Consideration::new("energy", n.energy, IDENTITY),
            ],
            0.0,
        ));
    }
    let n = world.comp::<crate::components::Needs>(id)?;
    if let Some(p) = world.jobs2.pitches.get(&id) {
        let hours = in_hours(world, p) && !crate::systems::health::sick(world, id);
        return Some((
            vec![
                Consideration::new("pitch hours", can(hours), GATE),
                Consideration::new("energy", n.energy, IDENTITY),
                Consideration::new("U(wealth)", urgency(n.wealth), Curve::Logistic { k: 8.0, mid: 0.5 }),
            ],
            0.0,
        ));
    }
    if corner_for(world, id).is_none() {
        // Round 4 T5 (OPEN-217): the search's door, after the dawn.
        return crate::systems::search::considerations(world, id);
    }
    Some((
        vec![
            Consideration::new("the corner at dawn", 1.0, GATE),
            // A hungry body eats before it queues.
            Consideration::new("hunger", n.hunger, IDENTITY),
            Consideration::new("energy", n.energy, IDENTITY),
            Consideration::new("U(wealth)", urgency(n.wealth), Curve::Logistic { k: 8.0, mid: 0.4 }),
        ],
        0.0,
    ))
}

/// The Pitch goal's scripted plan: `GoTo(Spot)` (the street tile) then `Vend`.
pub fn plan(world: &mut World, id: EntityId) -> Option<Plan> {
    street_tile(world, id)?;
    let step = |action| ActionInstance { action, target: None, tile: None };
    let steps = vec![step(ActionKind::GoTo(LocationKey::Spot)), step(ActionKind::Vend)];
    Some(Plan { goal: GoalKind::Pitch, target: None, steps, started_tick: world.tick })
}

/// A Pitch step may start: `Vend` on the street within `pitch_reach` of the tile.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind) -> bool {
    match kind {
        ActionKind::GoTo(_) => street_tile(world, id).is_some(),
        ActionKind::Vend => {
            let Some(t) = street_tile(world, id) else { return false };
            world
                .comp::<Position>(id)
                .is_some_and(|p| p.building.is_none() && p.tile.manhattan(t) <= world.config.jobs2.pitch_reach)
        }
        _ => false,
    }
}

/// `Vend`'s ticks: an hour at the Pitch; at the corner, to the dawn pick.
pub fn vend_ticks(world: &World, id: EntityId) -> Tick {
    if world.jobs2.pitches.contains_key(&id) {
        return TICKS_PER_HOUR;
    }
    // M18 P2: an ask at a bound door (a directed plan's) is the door ask's.
    let bound = world.comp::<Brain>(id).and_then(|b| b.current_step()).is_some_and(|s| s.target.is_some());
    if bound {
        return crate::systems::search::door_ask_ticks(world);
    }
    // Round 4 T5: a searcher's ask at a door (never in the dawn window).
    if corner_for(world, id).is_none() {
        if let Some(t) = crate::systems::search::ask_ticks(world, id) {
            return t;
        }
    }
    let h = world.config.jobs2.corner_hour;
    Tick::from(h.saturating_sub(world.tick_of_day()).max(1))
}

// ---------------------------------------------------------------------------
// The snapshot
// ---------------------------------------------------------------------------

/// The day's snapshot columns: the Pitches open by kind, the day labourers.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let mut by = [0u32; 3];
    for p in world.jobs2.pitches.values() {
        match kind_of_pitch(world, p) {
            Some(PitchKind::Vendor) => by[0] += 1,
            Some(PitchKind::Rickshaw) => by[1] += 1,
            Some(PitchKind::Tinker) => by[2] += 1,
            // Jobs & lives phase 4: the vice's own column (`street_pitches`).
            Some(PitchKind::Street) | None => {}
        }
    }
    let day = day_role(world).map_or(0, |r| world.workers(r).len() as u32);
    let row = ownership::jobs2_row(world);
    row.pitches = by.iter().sum();
    row.pitches_vendor = by[0];
    row.pitches_rickshaw = by[1];
    row.pitches_tinker = by[2];
    row.day_labourers = day;
}

/// The Pitch an agent owns (the inspector, the probes).
pub fn pitch_of(world: &World, id: EntityId) -> Option<&Pitch> {
    world.jobs2.pitches.get(&id)
}
