use super::*;

/// A gang's syndicate (a living parent of kind Syndicate holding it).
fn syndicate_of(world: &World, gang: EntityId) -> Option<OutsideId> {
    if !crate::systems::outside::on(world) {
        return None;
    }
    world
        .outside
        .factions
        .iter()
        .find(|f| {
            f.kind == OutsideKind::Syndicate && !f.dead && f.parent.as_ref().is_some_and(|p| p.branches.contains(&gang))
        })
        .map(|f| f.id)
}

/// Jobs & lives phase 3: a gang's living syndicate (`None` with the
/// outside off or none), which ships its crates.
pub fn syndicate_id(world: &World, gang: EntityId) -> Option<OutsideId> {
    syndicate_of(world, gang)
}

/// Jobs & lives phase 3: a parent's treasury abroad (0 for none).
pub fn treasury_abroad(world: &World, id: OutsideId) -> i64 {
    world.outside.faction(id).map_or(0, |f| f.treasury)
}

/// Jobs & lives phase 3: a parent pays a cost abroad (a crate shipped in
/// kind): out of its treasury and out of `minted`, booked as spent.
pub fn spend_abroad(world: &mut World, id: OutsideId, cost: i64) -> bool {
    pay_abroad(world, id, cost)
}

/// Does `gang` have a living syndicate (M12's bootstrap re-formation is
/// replaced by the syndicate's refound).
pub fn has_syndicate(world: &World, gang: EntityId) -> bool {
    syndicate_of(world, gang).is_some()
}

/// God `SendBrothers`: the gang's syndicate sends its brothers at the next
/// midnight, the cooldown ignored (the syndicate pays).
pub fn send_brothers(world: &mut World, gang: EntityId) -> Result<String, String> {
    let gname = world.owner_label(Some(gang));
    let id = syndicate_of(world, gang).ok_or(format!("SendBrothers: {gname} has no syndicate"))?;
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let f = world.outside.faction(id).ok_or("SendBrothers: no account".to_string())?;
    let pname = f.name.clone();
    if f.treasury < cfg.brother_cost * i64::from(cfg.brothers) {
        return Err(format!("SendBrothers: {pname} cannot pay"));
    }
    let p = parent_mut(world, id).ok_or(format!("SendBrothers: {pname} is no parent"))?;
    if p.brothers_due.iter().any(|d| d.gang == gang) {
        return Err(format!("SendBrothers: brothers already due for {gname}"));
    }
    p.brothers_due.push(BrothersDue { gang, at: now, killer: None, refound: false });
    p.last_brothers = Some(now);
    Ok(format!("{pname} will send {} brothers to {gname} at midnight", cfg.brothers))
}

/// § 4 brothers: a syndicate's gang gutted by a sack schedules `brothers`
/// bodies `brother_delay_days` on, unless inside the cooldown or the
/// syndicate cannot pay. True when scheduled.
pub fn schedule_brothers(world: &mut World, gang: EntityId, killer: Option<EntityId>) -> bool {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let Some(id) = syndicate_of(world, gang) else { return false };
    let Some(f) = world.outside.faction(id) else { return false };
    let Some(p) = f.parent.as_ref() else { return false };
    let cooled =
        p.last_brothers.is_none_or(|t| now.saturating_sub(t) >= u64::from(cfg.syndicate_cooldown_days) * TICKS_PER_DAY);
    let cost = cfg.brother_cost * i64::from(cfg.brothers);
    if !cooled || p.embargoed || f.treasury < cost || p.brothers_due.iter().any(|d| d.gang == gang) {
        return false;
    }
    if let Some(p) = parent_mut(world, id) {
        p.brothers_due.push(BrothersDue {
            gang,
            at: now + u64::from(cfg.brother_delay_days) * TICKS_PER_DAY,
            killer,
            refound: false,
        });
        p.last_brothers = Some(now);
    }
    true
}

/// A syndicate's midnight: a gang branch sacked since the last midnight
/// with fewer than `brothers_gutted_below` free members gets brothers on a
/// keyed roll under `brothers_p` (phase 4b: a decapitation is the gang's
/// own succession, `systems::succession`); an emptied gang past
/// `refound_days` with a fond enough syndicate (and the coins) is
/// refounded; the due brothers arrive (members at once, an Enemy edge to a
/// known killer).
pub fn syndicate_daily(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let Some(p) = parent(world, id) else { return };
    let gangs: Vec<EntityId> = p.branches.iter().copied().filter(|&g| world.has::<Gang>(g)).collect();
    let attitude = p.attitude;
    let window = u64::from(cfg.refound_window_days) * TICKS_PER_DAY;
    let recent_refounds = p.refounds.iter().filter(|&&t| now.saturating_sub(t) < window).count();
    // A gang gutted by a sack (an emptied one waits on its refound).
    let sacked_for = world.config.gangs.sacked_days * TICKS_PER_DAY;
    for &g in &gangs {
        let sacked_at = world
            .comp::<Gang>(g)
            .filter(|x| !x.members.is_empty())
            .and_then(|x| x.sacked_until)
            .map(|u| u.saturating_sub(sacked_for));
        if !sacked_at.is_some_and(|t| t <= now && now - t < TICKS_PER_DAY)
            || crate::systems::gang::fit_headcount(world, g) >= cfg.brothers_gutted_below
        {
            continue;
        }
        let u =
            crate::systems::econ::hash_unit(world.seed(), PURPOSE_BROTHERS, u64::from(g.index), now / TICKS_PER_DAY);
        if u < cfg.brothers_p {
            schedule_brothers(world, g, None);
        }
    }
    // Refounds.
    for &g in &gangs {
        let Some(x) = world.comp::<Gang>(g) else { continue };
        if !(x.emptied && x.members.is_empty()) {
            continue;
        }
        let waited =
            x.empty_since.is_some_and(|t| now.saturating_sub(t) >= u64::from(cfg.refound_days) * TICKS_PER_DAY);
        let need = cfg.refound_attitude + if recent_refounds > 0 { 0.3 } else { 0.0 };
        let Some(f) = world.outside.faction(id) else { continue };
        let due_already = f.parent.as_ref().is_some_and(|p| p.brothers_due.iter().any(|d| d.gang == g));
        if waited && attitude >= need && f.treasury >= cfg.refound_cost && !due_already {
            if let Some(p) = parent_mut(world, id) {
                p.brothers_due.push(BrothersDue { gang: g, at: now, killer: None, refound: true });
            }
        }
    }
    // Arrivals.
    let due: Vec<BrothersDue> = parent(world, id).map_or_else(Vec::new, |p| p.brothers_due.clone());
    let mut keep = Vec::new();
    for d in due {
        if d.at > now {
            keep.push(d);
            continue;
        }
        if !world.has::<Gang>(d.gang) {
            continue;
        }
        let cost = if d.refound { cfg.refound_cost } else { cfg.brother_cost * i64::from(cfg.brothers) };
        if !pay_abroad(world, id, cost) {
            continue;
        }
        let door =
            world.hideout_of(d.gang).and_then(|h| world.comp::<Building>(h).map(|bd| bd.door)).unwrap_or_default();
        let mut arrived = Vec::new();
        for _ in 0..cfg.brothers {
            let a = spawn_from_abroad(world, id, door);
            crate::systems::gang::enlist(world, a, d.gang);
            world
                .outside
                .outsiders
                .insert(a, Outsider { from: id, arrived: now, tour_until: u64::MAX, cohort: 0, brother: true });
            if let Some(k) = d.killer.filter(|&k| world.is_alive(k)) {
                let e = world.edge_entry(a, k);
                e.kind = RelKind::Enemy;
                e.affinity = -0.6;
                e.last_interaction = now;
            }
            arrived.push(a);
        }
        let n = arrived.len() as u32;
        world.outside.today.brothers += n;
        let (pname, gname) =
            (world.outside.faction(id).map_or_else(String::new, |f| f.name.clone()), world.owner_label(Some(d.gang)));
        if d.refound {
            world.outside.today.refounded += 1;
            if let Some(p) = parent_mut(world, id) {
                p.refounds.push(now);
                p.refounds.retain(|&t| now.saturating_sub(t) < window);
            }
            world.push_event(EventKind::GangRefounded, &arrived, format!("{pname} sent {n} to refound {gname}"));
        } else {
            world.push_event(EventKind::BrothersArrived, &arrived, format!("{n} brothers from {pname} joined {gname}"));
        }
    }
    let leaders: Vec<(EntityId, Option<EntityId>)> =
        gangs.iter().map(|&g| (g, world.comp::<Gang>(g).and_then(|x| x.leader))).collect();
    if let Some(p) = parent_mut(world, id) {
        p.brothers_due = keep;
        p.leaders = leaders;
    }
}
