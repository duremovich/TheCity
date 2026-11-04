use super::*;

pub(super) fn rows(world: &World, id: EntityId, verbs: impl IntoIterator<Item = Verb>) -> Vec<Row> {
    rows_with(world, id, verbs, |_| false)
}

/// The legal rows of `verbs`; a verb `grey` names that cannot be done now
/// is kept greyed with why (M18 P3), the rest dropped.
pub(super) fn rows_with(
    world: &World,
    id: EntityId,
    verbs: impl IntoIterator<Item = Verb>,
    grey: impl Fn(Verb) -> bool,
) -> Vec<Row> {
    // a body that cannot take an order (in cuffs, sentenced, down) lists nothing
    if refusal(world, id).is_some() {
        return vec![];
    }
    verbs
        .into_iter()
        .filter_map(|v| match plan_of(world, id, v) {
            Ok((_, label)) => Some(Row { verb: v, label, why_not: None }),
            Err(why) if grey(v) => Some(Row { verb: v, label: name_of_verb(world, v), why_not: Some(why) }),
            Err(_) => None,
        })
        .collect()
}

/// A greyed row's label: the verb's name without its numbers.
fn name_of_verb(world: &World, v: Verb) -> String {
    let n = |e: EntityId| world.name_of(e);
    match v {
        Verb::TakeBed(_) => "A bed for the night".into(),
        Verb::Apply(h) => format!("A place at {}", n(h)),
        Verb::AskWork(_) => "Ask about work".into(),
        Verb::Chat(o) => format!("Chat with {}", n(o)),
        Verb::Befriend(o) => format!("Befriend {}", n(o)),
        Verb::Court(o) => format!("Court {}", n(o)),
        Verb::Loan(o) => format!("Ask {} for a loan", n(o)),
        Verb::Threaten(o) => format!("Lean on {} for coins", n(o)),
        Verb::AskWorkOf(o) => format!("Ask {} for work", n(o)),
        // the mark's note is the greyed row's why already
        Verb::Take(c) => world.contracts.get(&c).map_or("a job".into(), |x| {
            let note = crate::systems::contracts::mark_note(world, x).map_or(String::new(), |w| format!(" ({w})"));
            job_line(world, x).replace(&note, "")
        }),
        Verb::Network(f) => format!("Get known at {}", n(f)),
        other => format!("{other:?}"),
    }
}

/// The HUD's rows: what the body can do wherever it stands.
pub fn menu_self(world: &World, id: EntityId) -> Menu {
    let verbs = [Verb::EatCarried, Verb::EatHome, Verb::SleepHome, Verb::SleepBed, Verb::SleepHere, Verb::Work];
    Menu { title: "you".into(), rows: rows(world, id, verbs) }
}

/// A click on building `b`: its door, then what can be done there.
pub fn menu_building(world: &World, id: EntityId, b: EntityId) -> Menu {
    let Some(bd) = world.comp::<Building>(b) else { return Menu::default() };
    let door = if opens(world, id, b) { Verb::GoIn(b) } else { Verb::GoToDoor(b) };
    let mut verbs = vec![door];
    match bd.kind {
        BuildingKind::Market => verbs.extend([Verb::BuyMeal(b), Verb::Groceries(b), Verb::Shoplift(b)]),
        BuildingKind::Home if home_of(world, id) == Some(b) => verbs.extend([Verb::EatHome, Verb::SleepHome]),
        BuildingKind::Home => verbs.extend([Verb::BreakIn(b), Verb::Extort(b), Verb::Squat(b)]),
        _ => verbs.push(Verb::Squat(b)),
    }
    verbs.extend([Verb::TakeBed(b), Verb::AskWork(b)]);
    // M18 P5h: a letting desk's places (the city's list at a Hall, a corp's
    // flats at its office, a gang's at its Hideout, a Block's own).
    verbs.extend(crate::systems::housing::desk_places(world, id, b).into_iter().map(Verb::Apply));
    let bed = crate::systems::street::booked_hotel(world, id) == Some(b)
        || world.comp::<Squatter>(id).is_some_and(|s| s.building == b);
    if bed {
        verbs.push(Verb::SleepBed);
    }
    if world.comp::<Job>(id).and_then(|j| j.employer) == Some(b) {
        verbs.push(Verb::Work);
    }
    // M18 P3: a Hotel's or a cot house's bed and a workplace's ask stay,
    // greyed with why, when they cannot be had (cots full, the post gone)
    let grey = |v: Verb| match v {
        Verb::TakeBed(h) => crate::systems::street::is_hotel(world, h),
        Verb::Apply(_) => true,
        Verb::AskWork(w) => {
            world.comp::<Building>(w).is_some_and(|x| world.config.buildings.for_kind(x.kind).staff > 0)
        }
        _ => false,
    };
    Menu { title: world.name_of(b), rows: rows_with(world, id, verbs, grey) }
}

/// A click on `tile` (`on_building`: a roof or a wall, the nearest within
/// `snap`): the menu of the building it names, every unit of a shop-house
/// that shares its rect merged (a Block over a Market over a Vat Farm), the
/// rows in id order. `None`: no building there (the street).
pub fn menu_at(world: &World, id: EntityId, tile: TilePos, on_building: bool, snap: u16) -> Option<Menu> {
    let first = if on_building { building_near(world, tile, snap) } else { building_at(world, tile) }?;
    let rect = world.comp::<Building>(first)?.rect;
    let mut units: Vec<EntityId> = world
        .buildings_by_kind()
        .values()
        .flatten()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|x| !x.demolished && x.rect == rect))
        .collect();
    units.sort_by_key(|b| b.index);
    // a vacant Lot sharing the rect (a big lot's unbuilt plots) has nothing to do at it
    let lot = |b: &EntityId| world.comp::<Building>(*b).is_some_and(|x| x.kind == BuildingKind::Lot);
    if units.iter().any(|b| !lot(b)) {
        units.retain(|b| !lot(b));
    }
    let menus: Vec<Menu> = units.iter().map(|&b| menu_building(world, id, b)).collect();
    let title = menus.iter().map(|m| m.title.clone()).collect::<Vec<_>>().join(" / ");
    let mut rows: Vec<Row> = menus.into_iter().flat_map(|m| m.rows).collect();
    // one door walk for the units of one rect (they share the street outside)
    let mut seen = Vec::new();
    let mut door = false;
    rows.retain(|r| {
        let walk = matches!(r.verb, Verb::GoToDoor(_));
        let keep = !seen.contains(&r.verb) && !(walk && door);
        door |= walk;
        seen.push(r.verb);
        keep
    });
    Some(Menu { title, rows })
}

/// A click on a body: the people verbs (a child: an empty menu, never a target).
pub fn menu_body(world: &World, id: EntityId, o: EntityId) -> Menu {
    let title = world.name_of(o);
    if !valid_person(world, id, o) {
        let child = world.has::<Identity>(o) && !demography::is_adult(world, o);
        return Menu { title: if child { format!("{title} (a child)") } else { title }, rows: vec![] };
    }
    // C5: a body knocked out hears nothing: walk over, go through its pockets
    if crate::systems::combat::knocked_out(world, o) {
        let title = format!("{title} (knocked out)");
        return Menu { title, rows: rows(world, id, [Verb::WalkOver(o), Verb::Rob(o)]) };
    }
    let verbs = [
        Verb::WalkOver(o),
        Verb::Chat(o),
        Verb::Befriend(o),
        Verb::Court(o),
        Verb::Loan(o),
        Verb::AskWorkOf(o),
        Verb::Threaten(o),
        Verb::Attack(o),
        Verb::Help(o),
        Verb::Rob(o),
    ];
    // M18 P3: the talk rows stay, greyed with why (too far, nothing to lend)
    let grey = |v: Verb| {
        matches!(
            v,
            Verb::Chat(_) | Verb::Befriend(_) | Verb::Court(_) | Verb::Loan(_) | Verb::AskWorkOf(_) | Verb::Threaten(_)
        )
    };
    Menu { title, rows: rows_with(world, id, verbs, grey) }
}

/// The human picked `verb`: its plan, made again now, runs as a directed
/// plan (Work through the planner, `plan::plan_for`).
pub fn act(world: &mut World, id: EntityId, verb: Verb) {
    let what = format!("{verb:?}");
    if let Some(why) = refusal(world, id) {
        return refuse(world, id, &what, &why);
    }
    match verb {
        Verb::Befriend(_) | Verb::Loan(_) | Verb::Threaten(_) | Verb::AskWorkOf(_) => {
            let name = world.name_of(id);
            match talk(world, id, verb) {
                Ok(line) => {
                    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: {line}"));
                }
                Err(why) => refuse(world, id, &what, &why),
            }
            return;
        }
        Verb::Answer(made, take) => return answer(world, id, made, take),
        // M18 P5: go for the witness who wants paying
        Verb::Resist(made) => {
            let Some(o) = offer_of(world, id, made) else { return refuse(world, id, &what, "the offer lapsed") };
            let OfferKind::Blackmail { witness, demand } = o.what else { return };
            if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
                p.offers.retain(|x| x.made != made);
            }
            take_wheel(world, id);
            if let Err(why) = crate::systems::consequence::blackmail_answer(world, witness, id, demand, 2) {
                refuse(world, id, &what, &why);
            }
            return;
        }
        // C5: the pockets of a body knocked out, at once (a Theft, witnessed)
        Verb::Rob(o) => {
            if let Err(why) = plan_of(world, id, verb) {
                return refuse(world, id, &what, &why);
            }
            take_wheel(world, id);
            crate::systems::combat::rob(world, id, o);
            return;
        }
        Verb::WorkJob => {
            let Some(c) = held_job(world, id) else { return refuse(world, id, &what, "no job held") };
            take_wheel(world, id);
            world.abort_plan(id);
            if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
                p.contract = Some(c);
            }
            return job_next(world, id);
        }
        _ => {}
    }
    let (plan, label) = match plan_of(world, id, verb) {
        Ok(p) => p,
        Err(why) => return refuse(world, id, &what, &why),
    };
    if let Verb::Take(c) = verb {
        let brokered = world.contracts.get(&c).is_some_and(|x| x.broker.is_some());
        let now = world.tick;
        if brokered {
            // the fixer holds it for him while he comes (`contracts::plan`'s claim)
            if let Some(x) = world.contracts.get_mut(&c) {
                x.claimed = Some((id, now));
            }
            install_directed(world, id, plan, label);
            if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
                p.contract = Some(c);
            }
            return;
        }
        take_wheel(world, id);
        world.abort_plan(id);
        if crate::systems::contracts::player_take(world, id, c) {
            if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
                p.contract = Some(c);
                p.doing = Some(label);
            }
            job_next(world, id);
        }
        return;
    }
    if let Verb::Network(_) = verb {
        return install_directed(world, id, plan, label);
    }
    if verb == Verb::Work {
        goal(world, id, GoalKind::Work);
        // M18 W: the directed Work runs the shift, plan after plan, to its
        // close or a Stop (`shift_next`), as an NPC's shift does.
        let key = world.comp::<Job>(id).map(|j| j.next_shift_key(world.tick));
        let planned = world.comp::<Brain>(id).is_some_and(|b| b.plan_goal() == Some(GoalKind::Work));
        if let Some(p) = world.comp_mut::<Brain>(id).and_then(|b| b.player.as_mut()) {
            p.doing = Some(label);
            p.shift = key.filter(|_| planned);
        }
        return;
    }
    take_wheel(world, id);
    world.abort_plan(id);
    let tick = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(GoalKind::Idle);
        b.goal_since = tick;
        if let Some(p) = b.player.as_mut() {
            p.doing = Some(label.clone());
        }
    }
    let line = steps_line(&plan.steps);
    // The shared things the plan uses are held as a planned NPC's are
    // (`plan::reserve_for`: the food, the partner); a bed by the Hotel the
    // menu named (its `bed_of` reads the NPC's keyed walk, not a picked one).
    let held = Plan {
        steps: plan.steps.iter().filter(|s| s.action != ActionKind::CheckIn).cloned().collect(),
        ..plan.clone()
    };
    plan::install(world, id, plan);
    plan::reserve_for(world, id, &held);
    // C4: the rescuer's claim on the downed (the Rescue's own `plan`).
    if let Verb::Help(o) = verb {
        if let Some(x) = world.wounded.get_mut(&o) {
            x.rescuer = Some(id);
        }
    }
    if let Verb::TakeBed(h) = verb {
        let until = tick + world.config.brain.plan_timeout_ticks;
        world.reserve(id, crate::exec::reservations::ReservationKind::Bed { home: h }, until);
    }
    let name = world.name_of(id);
    world.push_event(EventKind::PlayerAction, &[id], format!("{name}: {label} [{line}]"));
}

// ---------------------------------------------------------------------------
// M18 P5: start and loop (the new nobody, autopilot as safe idle)
// ---------------------------------------------------------------------------

/// A body the human holds: driven, or on autopilot.
pub fn possessed(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).is_some_and(|b| b.player.is_some())
}

/// P5 (Dylan 2026-11-03): autopilot is safe idle. The body keeps itself
/// alive (eats, sleeps, works the job it holds, flees or yields when
/// attacked) and changes nothing of its story: no new job, gang, contract,
/// offer, courtship, theft or fight. A filter on the NPC brain's goals
/// (`safe_goal`, `utility::think_scored`) and its steps
/// (`ActionKind::safe_idle_ok`, the planner's and exec's preconditions), and
/// the city's offers wait for the human (`care::covers`, `underwater`'s draw).
pub fn safe_idle(world: &World, id: EntityId) -> bool {
    world.comp::<Brain>(id).and_then(|b| b.player.as_ref()).is_some_and(|p| p.autopilot)
}

/// The goals a safe-idle body may pursue.
pub fn safe_goal(goal: GoalKind) -> bool {
    matches!(
        goal,
        GoalKind::Eat
            | GoalKind::Sleep
            | GoalKind::Work
            | GoalKind::Earn
            | GoalKind::Flee
            | GoalKind::Treat
            | GoalKind::Idle
    )
}
