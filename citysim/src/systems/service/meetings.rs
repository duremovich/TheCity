use super::*;

// ---------------------------------------------------------------------------
// Lives at work phase 5 (§ 2.4, § 2.2's civic clerk): the tower and the brokers
// ---------------------------------------------------------------------------

/// A paper buyer queued at midnight for a Fixer's open hours
/// (`World::walk_ins`, saved while pending): the forger the midnight pass
/// named (paid as `gray::papers_daily` paid him), the tier and the day queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkIn {
    pub buyer: EntityId,
    pub fixer: EntityId,
    pub forger: EntityId,
    pub tier: u8,
    pub day: u64,
    /// The pre-view pass phase 4 (P3, OPEN-119): the order is paid at the
    /// handover, not at midnight (written only when set).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unpaid: bool,
    /// Round 5 U5 (OPEN-217): what the forger made (papers, or a forged
    /// weapons licence; written only when not papers).
    #[serde(default, skip_serializing_if = "WalkKind::is_papers")]
    pub kind: WalkKind,
}

/// Round 5 U5 (OPEN-217): a forger's commission: papers for the wanted, or
/// a forged weapons licence (`[gray] licence_price`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalkKind {
    #[default]
    Papers,
    Licence,
}

impl WalkKind {
    pub fn is_papers(&self) -> bool {
        *self == WalkKind::Papers
    }
}

/// Is `id` a graded member of staff (its post graded, or its base grade)?
fn graded(world: &World, id: EntityId) -> bool {
    world
        .comp::<Job>(id)
        .is_some_and(|j| j.grade >= 1 || crate::systems::classpay::base_grade(world, j.employer, j.role) >= 1)
}

/// § 6: the exec at the table: alive and free; a body inside the Hq, a
/// Statistical exec credited in the office hours (the caller's clock).
fn exec_present(world: &World, exec: EntityId, hq: EntityId) -> bool {
    if !crate::systems::law::living(world, exec) || world.has::<Sentence>(exec) || world.has::<Job>(exec) {
        return false;
    }
    match world.comp::<Brain>(exec).map(|b| b.lod) {
        Some(Lod::Statistical) => true,
        Some(_) => world.comp::<Position>(exec).is_some_and(|p| p.building == Some(hq)),
        None => false,
    }
}

/// § 2.4 (exec): each hour of the office shift on a workday, each corp's
/// exec at its Hq (`life::hq_of`) meets up to `meeting_attendees` of the
/// graded staff present there (the lowest `hash(PURPOSE_TABLE, exec,
/// staff, hour)`): an `Attendee` encounter through the `exec` row, both
/// ways. The table is kept for the decision (`World::meeting_tables`).
pub(super) fn meeting_hour(world: &mut World) {
    let tod = world.tick_of_day();
    let (s, e) = world.config.life.exec_shift;
    if tod < s || tod >= e || !crate::exec::routine::is_workday(world.day() as i64) {
        return;
    }
    let hour = world.tick / TICKS_PER_HOUR;
    let cap = usize::from(world.config.service.meeting_attendees);
    for corp in world.corps() {
        let Some(exec) = world.comp::<crate::components::Corp>(corp).and_then(|c| c.exec) else { continue };
        let Some(hq) = crate::systems::life::office_of(world, corp, exec) else { continue };
        if !exec_present(world, exec, hq) {
            continue;
        }
        // M18 W: the exec's office hour is a seam a work beat may fill.
        if crate::systems::workbeats::on(world) {
            let day = world.day() as i64;
            crate::systems::workbeats::seam(world, exec, hq, None, crate::config::BeatAt::Hour, day);
        }
        let mut staff: Vec<(f32, EntityId)> = world
            .staff_of(hq)
            .iter()
            .copied()
            .filter(|&a| a != exec && graded(world, a) && present(world, a, hq))
            .map(|a| (hash(world, PURPOSE_TABLE, key(exec) ^ key(a), hour), a))
            .collect();
        staff.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        staff.truncate(cap);
        if staff.is_empty() {
            continue;
        }
        {
            let r = ownership::jobs2_row(world);
            r.meetings_held += 1;
            r.meeting_attendees += staff.len() as u32;
        }
        let d = world.district_of_building(hq);
        for &(_, a) in &staff {
            encounter_post(world, "exec", exec, a, Some(hq), d, ServeKind::Attendee, 0);
        }
        world.meeting_tables.insert(corp, staff.into_iter().map(|(_, a)| a).collect());
    }
}

/// § 2.4 (exec), from `corp_brain::rescore` after `attention::tilt`: when
/// the two best orders lie within `decision_margin` and one is expansive,
/// the other defensive, the corp's exec (alive and free) picks: the
/// expansive on `hash(PURPOSE_DECIDE, exec, day) < 0.5 + 0.5 × (courage −
/// loyalty)`, else the defensive; the picked one gains the margin and the
/// scores are re-sorted (stable, best first). A hash of the exec's traits,
/// the same in any tier (§ 6). Returns the exec when he made the call.
pub fn decide(world: &World, corp: EntityId, scores: &mut [crate::components::CorpOrderScore]) -> Option<EntityId> {
    if !tower_on(world) || scores.len() < 2 {
        return None;
    }
    let cfg = &world.config.service;
    let margin = cfg.decision_margin;
    if margin <= 0.0 || scores[0].score - scores[1].score > margin {
        return None;
    }
    let class = |o: crate::components::CorpOrder| {
        if cfg.expansive.contains(&o) {
            Some(true)
        } else if cfg.defensive.contains(&o) {
            Some(false)
        } else {
            None
        }
    };
    let (exp, def) = match (class(scores[0].order), class(scores[1].order)) {
        (Some(true), Some(false)) => (0, 1),
        (Some(false), Some(true)) => (1, 0),
        _ => return None,
    };
    let exec = world.comp::<crate::components::Corp>(corp).and_then(|c| c.exec)?;
    if !crate::systems::law::living(world, exec) || world.has::<Sentence>(exec) {
        return None;
    }
    let p = world
        .comp::<crate::components::Personality>(exec)
        .map_or(0.5, |t| (0.5 + 0.5 * (t.courage - t.loyalty)).clamp(0.0, 1.0));
    let pick = if hash(world, PURPOSE_DECIDE, key(exec), world.day()) < p { exp } else { def };
    scores[pick].score += margin;
    scores.sort_by(|a, b| b.score.total_cmp(&a.score));
    Some(exec)
}

/// § 2.4 (exec): the call changed the corp's outcome: `decisions`, an
/// `EventKind::Served` naming the table (the corp's last Meeting), each
/// attendee's `Served` memory of the exec and the `Decided` deed posted
/// (§ 3.6: the attendees' gossip).
pub fn decided(world: &mut World, corp: EntityId, exec: EntityId, what: String) {
    ownership::jobs2_row(world).decisions += 1;
    let table: Vec<EntityId> = world
        .meeting_tables
        .get(&corp)
        .map(|t| t.iter().copied().filter(|&a| crate::systems::law::living(world, a)).collect())
        .unwrap_or_default();
    let names: Vec<String> = table.iter().map(|&a| world.name_of(a)).collect();
    let at = if names.is_empty() { "nobody".to_string() } else { names.join(", ") };
    let corp_name = world.comp::<crate::components::Corp>(corp).map_or_else(|| world.name_of(corp), |c| c.name.clone());
    // Round 4 T6: a call made off the office hours or away from the Hq (a
    // shock rescore at night) is made from where he is; the table is the
    // last one that sat.
    let tod = world.tick_of_day();
    let (s, e) = world.config.life.exec_shift;
    let sitting = tod >= s
        && tod < e
        && crate::exec::routine::is_workday(world.day() as i64)
        && crate::systems::life::office_of(world, corp, exec).is_some_and(|hq| exec_present(world, exec, hq));
    let seat = if sitting {
        format!("at the table: {at}")
    } else {
        // A body's building names where the call was made; off screen
        // (Statistical) the place is not kept.
        let body = world.comp::<Brain>(exec).is_some_and(|b| b.lod != Lod::Statistical);
        let from = world
            .comp::<Position>(exec)
            .and_then(|p| p.building)
            .filter(|_| body)
            .map_or_else(|| "off the table".to_string(), |b| format!("called from {}", world.name_of(b)));
        if names.is_empty() {
            from
        } else {
            format!("{from}; the table last sat: {at}")
        }
    };
    let text = format!("{} decided {corp_name} would {what}; {seat}", world.name_of(exec));
    let mut actors = vec![exec, corp];
    actors.extend(table.iter().copied());
    world.push_event(EventKind::Served, &actors, text);
    let salience = world.config.service.served_salience;
    for &a in &table {
        world.remember(a, MemoryKind::Served, Some(exec), salience, 0.0, false);
    }
    let d =
        crate::systems::life::office_of(world, corp, exec).map(|h| world.district_of_building(h)).unwrap_or_default();
    crate::systems::gossip::post_deed(world, d, crate::word::Deed::Decided, Some(exec), Some(corp));
}

/// The Fixer's open hours now: a member of its staff on shift this hour on
/// a workday (the forger's and the Fixer's day shift).
fn fixer_open_now(world: &World, fixer: EntityId) -> bool {
    let tod = world.tick_of_day();
    world.staff_of(fixer).iter().any(|&s| {
        world.comp::<Job>(s).is_some_and(|j| {
            j.on_shift(tod) && crate::exec::routine::workday_of(world, s, j, j.shift_key_at(world.tick))
        })
    })
}

/// § 2.4 (fixer): the owner on duty without a post: alive, free, holding
/// no Job; a body inside his Fixer, a Statistical owner credited in its
/// open hours (deviation, as the pimp's: § 6's `shift_attended` needs a Job).
fn owner_present(world: &World, fixer: EntityId) -> Option<EntityId> {
    let o = world.owner_of(fixer)?;
    if !world.has::<Brain>(o)
        || world.has::<Job>(o)
        || world.has::<Sentence>(o)
        || !crate::systems::law::living(world, o)
    {
        return None;
    }
    let here = match world.comp::<Brain>(o).map(|b| b.lod) {
        Some(Lod::Statistical) => fixer_open_now(world, fixer),
        _ => world.comp::<Position>(o).is_some_and(|p| p.building == Some(fixer)),
    };
    here.then_some(o)
}

/// Round 9 Z2: the one behind `fixer`'s desk for `served` now (a server on
/// duty with room this hour, else the owner in), `None` with the desk empty:
/// a walk-in takes his job from him.
pub fn desk_server(world: &World, fixer: EntityId, served: EntityId) -> Option<EntityId> {
    fixer_server(world, fixer, served)
}

/// § 2.4 (fixer): who serves `served` at `fixer` now: the on-duty `Client`
/// servers (the Fixer's staff, the forger) with room this hour by the
/// lowest `hash(PURPOSE_FIXER, …)` (D6), else the owner present. `None`:
/// nobody on duty.
pub(super) fn fixer_server(world: &World, fixer: EntityId, served: EntityId) -> Option<EntityId> {
    let hour = world.tick / TICKS_PER_HOUR;
    servers(world, fixer, ServeKind::Client)
        .into_iter()
        .filter(|&(s, cap)| s != served && world.service_room.get(&s).is_none_or(|r| r.hour != hour || r.taken < cap))
        .map(|(s, _)| (hash(world, PURPOSE_FIXER, key(fixer) ^ key(served), hour ^ key(s)), s))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, s)| s)
        .or_else(|| owner_present(world, fixer).filter(|&o| o != served))
}

/// The fixer a party off the hours is credited to (§ 2.4: "credited
/// world-side at that hour"; the posts come at midnight and the takes at
/// the match hour, outside the day shift): the Fixer's `Client` staff
/// alive and free by the lowest `hash(PURPOSE_FIXER, …)`, else the owner
/// alive and free. Review fix 7: a staff server with room left this hour
/// (his row's `per_hour`, as the on-duty pick reads it); all full, the
/// owner (whose post has no room ledger), else nobody (the contract
/// stands, the encounter is not credited).
pub(super) fn fixer_book(world: &World, fixer: EntityId, party: EntityId) -> Option<EntityId> {
    let hour = world.tick / TICKS_PER_HOUR;
    let free = |s: EntityId| s != party && crate::systems::law::living(world, s) && !world.has::<Sentence>(s);
    let room = |s: EntityId, cap: u8| world.service_room.get(&s).is_none_or(|r| r.hour != hour || r.taken < cap);
    world
        .staff_of(fixer)
        .iter()
        .copied()
        .filter(|&s| {
            free(s) && row_of_agent(world, s).is_some_and(|r| r.kind == ServeKind::Client && room(s, r.per_hour))
        })
        .map(|s| (hash(world, PURPOSE_FIXER, key(fixer) ^ key(party), hour ^ key(s)), s))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, s)| s)
        // The owner on his own post here is one of the staff above (full).
        .or_else(|| {
            world.owner_of(fixer).filter(|&o| {
                world.has::<Brain>(o) && free(o) && world.comp::<Job>(o).is_none_or(|j| j.employer != Some(fixer))
            })
        })
}

/// Round 5 U5 (OPEN-217): the fixer who runs `fixer`'s book, told of its
/// posts, takes and outcomes (`contracts`' events name him): a holder of
/// its `Fixer` post alive and free, a body first (the camera's; round 4
/// T4: an off-screen colleague named by id took a body's work), then the
/// lowest id; else its owner, a person alive and free. `None` with
/// neither (an empty office, a corp's or a gang's).
pub fn fixer_hand(world: &World, fixer: EntityId) -> Option<EntityId> {
    let free = |s: EntityId| crate::systems::law::living(world, s) && !world.has::<Sentence>(s);
    world
        .staff_of(fixer)
        .iter()
        .copied()
        .filter(|&s| free(s) && world.comp::<Job>(s).is_some_and(|j| j.role == crate::components::Role::Fixer))
        .min_by_key(|&s| (!crate::systems::presence::is_body(world, s), s))
        .or_else(|| world.owner_of(fixer).filter(|&o| world.has::<Brain>(o) && free(o)))
}

/// The `Client` encounter at the Fixer: a staff server through his row
/// (the room counted), the owner through the `Fixer` row.
pub(super) fn fixer_meet(world: &mut World, server: EntityId, served: EntityId, fixer: EntityId, coins: i64) {
    let d = world.district_of_building(fixer);
    let staff = world.comp::<Job>(server).is_some_and(|j| j.employer == Some(fixer));
    if staff {
        let hour = world.tick / TICKS_PER_HOUR;
        let r = world.service_room.entry(server).or_default();
        if r.hour != hour {
            *r = Room { hour, taken: 0 };
        }
        // Both pickers leave room under the cap; saturating, never wrapping.
        r.taken = r.taken.saturating_add(1);
        encounter(world, server, served, Some(fixer), d, ServeKind::Client, coins);
    } else {
        encounter_post(world, "Fixer", server, served, Some(fixer), d, ServeKind::Client, coins);
    }
}
