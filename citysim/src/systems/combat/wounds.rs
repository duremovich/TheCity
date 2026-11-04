use super::*;

/// C4: a robot's part where a hit lands (the line's machine words).
pub(super) fn robot_part(r: Region) -> &'static str {
    match r {
        Region::Head => "sensor head",
        Region::Torso => "chassis",
        Region::ArmL | Region::ArmR => "arm",
        Region::LegL | Region::LegR => "leg",
    }
}

/// C4: what a hit does to a robot, by severity.
pub(super) fn robot_harm(v: Severity) -> &'static str {
    match v {
        Severity::Graze => "scratched",
        Severity::Wound => "dented",
        Severity::Grave => "smashed",
    }
}

// ---------------------------------------------------------------------------
// C2: wounds that last (docs/COMBAT.md § 2)
// ---------------------------------------------------------------------------

/// The tick `x` heals (an old save's wound: `since` + its days).
pub(super) fn heal_at(x: &Wound, c: &crate::config::CombatCfg) -> Tick {
    if x.heal_at > 0 {
        x.heal_at
    } else {
        x.since + Tick::from(c.heal_days[x.severity.index()]) * TICKS_PER_DAY
    }
}

/// Seen to: the bleeding stopped, the days left cut to `treated_heal_mult`.
fn treat_one(x: &mut Wound, now: Tick, c: &crate::config::CombatCfg) {
    if x.treated {
        return;
    }
    let at = heal_at(x, c);
    x.treated = true;
    x.bleeding = false;
    x.heal_at = if at > now { now + ((at - now) as f32 * c.treated_heal_mult.clamp(0.0, 1.0)) as Tick } else { at };
}

/// The worst untreated open wound on `id` of `[combat] treat_from` or
/// worse: what sends it to a Clinic (`customers::hurt`).
pub fn worst_untreated(world: &World, id: EntityId) -> Option<Severity> {
    let w = world.comp::<Wounds>(id)?;
    let from = world.config.combat.treat_from.min(2);
    w.list.iter().filter(|x| !x.treated && x.severity.index() >= from).map(|x| x.severity).max()
}

/// The Treat goal's hurt term for `id`'s wounds (`treat_urgency`).
/// C3b: it grows `treat_urgency_day` a day the oldest such wound has gone
/// unseen (to 1).
pub fn urgency(world: &World, id: EntityId) -> f32 {
    let Some(s) = worst_untreated(world, id) else { return 0.0 };
    let c = &world.config.combat;
    let from = c.treat_from.min(2);
    let oldest = world
        .comp::<Wounds>(id)
        .and_then(|w| w.list.iter().filter(|x| !x.treated && x.severity.index() >= from).map(|x| x.since).min())
        .unwrap_or(world.tick);
    let days = world.tick.saturating_sub(oldest) as f32 / TICKS_PER_DAY as f32;
    (c.treat_urgency[s.index()] + c.treat_urgency_day * days).min(1.0)
}

/// The price multiplier on a patch-up for `id`'s worst untreated wound.
pub fn price_mult(world: &World, id: EntityId) -> f32 {
    worst_untreated(world, id).map_or(1.0, |s| world.config.combat.treat_price_mult[s.index()])
}

/// A doctor sees to `id`'s wounds (a patch-up at a Clinic or the back
/// room): every open wound treated. Returns what was seen to ("the left
/// leg, the body"), `None` with nothing to treat.
pub fn treat(world: &mut World, id: EntityId) -> Option<String> {
    let now = world.tick;
    let c = world.config.combat.clone();
    let w = world.comp_mut::<Wounds>(id)?;
    let mut seen: Vec<&'static str> = Vec::new();
    for x in w.list.iter_mut().filter(|x| !x.treated) {
        treat_one(x, now, &c);
        if x.severity >= Severity::Wound && !seen.contains(&x.region.label()) {
            seen.push(x.region.label());
        }
    }
    (!seen.is_empty()).then(|| format!("the {}", seen.join(", the ")))
}

/// Why `id`'s wounds keep it from `job`'s shift: a Grave head wound from
/// any post; an arm or leg Wound or worse from a manual one (`[combat]
/// manual_posts`). `None`: fit to work.
pub fn unfit(world: &World, id: EntityId, job: &Job) -> Option<&'static str> {
    let w = world.comp::<Wounds>(id)?;
    if w.list.is_empty() {
        return None;
    }
    if w.open(Region::Head) == Some(Severity::Grave) {
        return Some("a head wound");
    }
    let arm = w.list.iter().any(|x| x.region.is_arm() && x.severity >= Severity::Wound);
    let leg = w.list.iter().any(|x| x.region.is_leg() && x.severity >= Severity::Wound);
    if !(arm || leg) {
        return None;
    }
    let role = role_key(job.role);
    if !world.config.combat.manual_posts.contains(&role) {
        return None;
    }
    Some(if leg { "a bad leg" } else { "a bad arm" })
}

fn slot_of(region: Region) -> Option<Slot> {
    if region.is_arm() {
        Some(Slot::Arms)
    } else if region.is_leg() {
        Some(Slot::Legs)
    } else {
        None
    }
}

/// `id` loses the limb at `region` (`why` for the line): a `Lost` mark (the
/// region reads as a Grave) and `[ages]`'s maim at its top level in the
/// limb's slot, until a ripperdoc fits chrome there (`ages` prosthetics,
/// [`fitted`]).
pub fn lose_limb(world: &mut World, id: EntityId, region: Region, why: &str) {
    if crate::systems::ages::is_protected(world, id) || !crate::systems::law::living(world, id) {
        return;
    }
    // A chrome limb there takes it (C3: the hit wrecks the chrome).
    if slot_of(region).is_some_and(|s| crate::systems::chrome::implant_in(world, id, s).is_some()) {
        return;
    }
    if !world.has::<Wounds>(id) {
        world.insert(id, Wounds::default());
    }
    let Some(w) = world.comp_mut::<Wounds>(id) else { return };
    if w.has_mark(region, Mark::Lost) {
        return;
    }
    w.marks.retain(|m| !(m.0 == region && m.1 == Mark::Maimed));
    w.marks.push((region, Mark::Lost));
    let text = format!("lost the {}: {why}", region.label());
    crate::systems::ages::maim_to(world, id, slot_of(region), &text, 3);
    if region.is_leg() {
        crate::systems::assets::rekit(world, id);
    }
}

/// A Grave on `region` healed badly: a `Maimed` mark (the region a Wound
/// for good) and `[ages]`'s maim in its slot.
fn maim_region(world: &mut World, id: EntityId, region: Region, treated: bool) {
    if slot_of(region).is_some_and(|s| crate::systems::chrome::implant_in(world, id, s).is_some()) {
        return;
    }
    let Some(w) = world.comp_mut::<Wounds>(id) else { return };
    if w.has_mark(region, Mark::Maimed) || w.has_mark(region, Mark::Lost) {
        return;
    }
    w.marks.push((region, Mark::Maimed));
    let why = if treated {
        format!("a {} wound that healed badly", region.label())
    } else {
        format!("a {} wound never seen to, healed badly", region.label())
    };
    crate::systems::ages::maim_to(world, id, slot_of(region), &why, 1);
    if region.is_leg() {
        crate::systems::assets::rekit(world, id);
    }
}

/// A chrome implant went into `slot` (`chrome::installed_now`): a lost or
/// maimed limb there is chrome now (its marks cleared, the line).
pub fn fitted(world: &mut World, id: EntityId, slot: Slot) {
    let regions: [Region; 2] = match slot {
        Slot::Arms => [Region::ArmL, Region::ArmR],
        Slot::Legs => [Region::LegL, Region::LegR],
        _ => return,
    };
    let Some(w) = world.comp_mut::<Wounds>(id) else { return };
    let mut was: Vec<(Region, Mark)> = Vec::new();
    w.marks.retain(|m| {
        let gone = regions.contains(&m.0) && m.1 != Mark::Scar;
        if gone {
            was.push(*m);
        }
        !gone
    });
    if was.is_empty() {
        return;
    }
    if w.list.is_empty() && w.marks.is_empty() && w.blood >= 1.0 {
        world.remove::<Wounds>(id);
        world.hurt.remove(&id);
    }
    crate::systems::assets::rekit(world, id);
    let parts: Vec<String> = was.iter().map(|(r, m)| format!("{} {}", m.label(), r.label())).collect();
    let text = format!("{}'s {} replaced with chrome", world.name_of(id), parts.join(" and "));
    world.push_event(EventKind::Fight, &[id], text);
}

/// Off screen (`lod::stat_hit`, docs/COMBAT.md § 2): a beating's wounds,
/// `stat_hits` hits from `stat_kind`'s damage table on `region_weights`,
/// and (`grave`, a killing survived) one Grave from `stat_grave_kind`'s,
/// which may take a limb (`sever_p`). Not bleeding (the off-screen fight's
/// blood is the split's) and not seen to: a body promoted later limps, and
/// the walk-in care ([`worst_untreated`]) can see to it.
pub fn stat_wounds(world: &mut World, id: EntityId, grave: bool) {
    if crate::systems::ages::is_protected(world, id) || !crate::systems::law::living(world, id) {
        return;
    }
    let c = world.config.combat.clone();
    let row = |k: &str| world.config.weapon_kinds.iter().find(|r| r.id == k).cloned().unwrap_or_default();
    let (beat, gravek) = (row(&c.stat_kind), row(&c.stat_grave_kind));
    let salt = world.tick;
    let [lo, hi] = c.stat_hits;
    let span = hi.saturating_sub(lo) + 1;
    let n = lo + ((hash(world, PURPOSE_STAT, key(id), salt) * span as f32) as u32).min(span - 1);
    for k in 0..n {
        let r = Region::ALL
            [pick_weighted(hash(world, PURPOSE_STAT, key(id), salt ^ (u64::from(k) + 1) << 40), &c.region_weights)];
        let sev = [Severity::Graze, Severity::Wound, Severity::Grave]
            [pick_weighted(hash(world, PURPOSE_STAT, key(id), salt ^ (u64::from(k) + 1) << 48), &beat.damage)];
        add_wound(world, id, r, sev, None, false, true);
    }
    if grave {
        let r = Region::ALL[pick_weighted(hash(world, PURPOSE_STAT, key(id), salt ^ 0xA1), &c.region_weights)];
        add_wound(world, id, r, Severity::Grave, None, false, true);
        if (r.is_arm() || r.is_leg())
            && !gravek.blunt
            && hash(world, PURPOSE_STAT, key(id), salt ^ 0xA2) < gravek.sever_p
        {
            lose_limb(world, id, r, &format!("cut off with a {}, off the street", gravek.id));
        }
    }
}

/// First aid: a friend, kin or a gang-mate of `id` (an adult on its feet,
/// free, not the one who did it) beside the body: in its building, or
/// within `aid_tiles` on the street.
fn aid_by(world: &World, id: EntityId, w: &Wounds) -> Option<EntityId> {
    use crate::components::RelKind;
    let at = world.comp::<Position>(id)?;
    let reach = world.config.combat.aid_tiles;
    let by: Vec<EntityId> = w.list.iter().filter_map(|x| x.by).collect();
    let ok = |o: EntityId| {
        o != id
            && !by.contains(&o)
            && crate::systems::law::living(world, o)
            && crate::systems::demography::is_adult(world, o)
            && !crate::systems::wounded::is_down(world, o)
            && !world.has::<crate::components::Sentence>(o)
            && world.comp::<Position>(o).is_some_and(|p| match (p.building, at.building) {
                (Some(x), Some(y)) => x == y,
                (None, None) => crate::systems::law::chebyshev(p.tile, at.tile) <= reach,
                _ => false,
            })
    };
    let tied = world.neighbours(id).filter(|&o| {
        world
            .edge(id, o)
            .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend))
    });
    let mut cands: Vec<EntityId> = tied.collect();
    if let Some(g) = world.gang_of(id).and_then(|g| world.comp::<Gang>(g)) {
        cands.extend(g.members.iter().copied());
    }
    cands.sort_unstable();
    cands.dedup();
    cands.into_iter().find(|&o| ok(o))
}

/// What a wound that healed leaves (decided in the pass, applied after).
enum Left {
    Scar(Region),
    Maim(Region, bool),
    Lose(Region),
}

/// Hourly (and seeding and training at midnight): every hurt body's
/// bleeding drains its blood (first aid stops a Wound's and slows a
/// Grave's; a Wound may stop on a roll; a Grave bleeds until a Clinic
/// treats it); at no blood it goes down; a body not bleeding gets blood
/// back; a wound heals at its `heal_at` (a scar, a maim, a lost limb by the
/// rolls); a manual worker with a bad arm or leg is turned away at his
/// shift's first hour; a wound not seen to says why.
pub fn hourly(world: &mut World) {
    if crate::systems::midnight::due(world, crate::systems::midnight::Mid::Tail) {
        seed_all(world);
    }
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) || world.hurt.is_empty() {
        return;
    }
    let now = world.tick;
    let tod = crate::time::tick_of_day(now);
    let c = world.config.combat.clone();
    let ids: Vec<EntityId> = world.hurt.iter().copied().collect();
    for id in ids {
        if !crate::systems::law::living(world, id) || !world.has::<Wounds>(id) {
            world.hurt.remove(&id);
            world.remove::<Wounds>(id);
            crate::systems::assets::rekit(world, id);
            continue;
        }
        let down = world.wounded.get(&id).map(|w| w.stable);
        let Some(mut w) = world.comp::<Wounds>(id).cloned() else { continue };
        let mut lines: Vec<String> = Vec::new();
        let name = world.name_of(id);
        // Held in the cells (the downed state lets go of the sentenced): the
        // jail dresses its wounds, it does not lie down there.
        let held = world.has::<crate::components::Sentence>(id);
        if held && w.list.iter().any(|x| !x.treated) {
            for x in w.list.iter_mut() {
                treat_one(x, now, &c);
            }
            w.blood = w.blood.max(0.3);
            lines.push(format!("{name}'s wounds were dressed in the cells"));
        }
        match down {
            // Stabilised at a Clinic: every wound seen to.
            Some(true) => {
                for x in w.list.iter_mut() {
                    treat_one(x, now, &c);
                }
                w.blood = w.blood.max(0.3);
            }
            // Down and bleeding: the M16b timer has it.
            Some(false) => {}
            None => {
                // First aid from a friend, kin or a gang-mate beside him.
                if w.list.iter().any(|x| x.bleeding && !x.aided) {
                    if let Some(h) = aid_by(world, id, &w) {
                        let mut parts: Vec<&'static str> = Vec::new();
                        let mut grave = false;
                        for x in w.list.iter_mut().filter(|x| x.bleeding && !x.aided) {
                            x.aided = true;
                            if x.severity < Severity::Grave {
                                x.bleeding = false;
                            } else {
                                grave = true;
                            }
                            parts.push(x.region.label());
                        }
                        let tail =
                            if grave { ": the bleeding slowed, it needs a Clinic" } else { ": the bleeding stopped" };
                        lines.push(format!("{} bound {name}'s {}{tail}", world.name_of(h), parts.join(" and ")));
                    }
                }
                for (k, x) in w.list.iter_mut().enumerate() {
                    if x.bleeding && x.severity < Severity::Grave {
                        let r = hash(world, PURPOSE_BLEED, key(id), now ^ (k as u64) << 48);
                        if x.severity == Severity::Graze || r < c.stop_p_wound {
                            x.bleeding = false;
                            lines.push(format!("the bleeding from {name}'s {} stopped", x.region.label()));
                        }
                    }
                }
                let drain: f32 = w
                    .list
                    .iter()
                    .filter(|x| x.bleeding)
                    .map(|x| c.bleed_hour[x.severity.index()] * if x.aided { c.aid_grave_mult } else { 1.0 })
                    .sum();
                if drain > 0.0 {
                    w.blood = (w.blood - drain).max(0.0);
                } else {
                    w.blood = (w.blood + c.regen_hour).min(1.0);
                }
            }
        }
        // Healing: a wound not bleeding is gone at its `heal_at`; a Wound or
        // worse may leave a scar, a Grave a maim or (a limb never seen to)
        // the limb.
        let before = w.list.len();
        let lost: Vec<Region> = w.marks.iter().filter(|m| m.1 == Mark::Lost).map(|m| m.0).collect();
        let mut left: Vec<Left> = Vec::new();
        w.list.retain(|x| {
            let gone = !x.bleeding && now >= heal_at(x, &c);
            if !gone || x.severity < Severity::Wound {
                return !gone;
            }
            let roll = |n: u64| hash(world, PURPOSE_HEAL, key(id), x.since ^ (n << 56) ^ (x.region as u64) << 48);
            let limb = x.region.is_arm() || x.region.is_leg();
            let mut how = "has healed";
            if x.severity == Severity::Grave && !lost.contains(&x.region) {
                if limb && !x.treated && roll(1) < c.rot_p {
                    left.push(Left::Lose(x.region));
                    how = "festered";
                } else if roll(2) < if x.treated { c.maim_p_treated } else { c.maim_p } {
                    left.push(Left::Maim(x.region, x.treated));
                    how = "has healed, badly";
                }
            }
            if roll(3) < c.scar_p[x.severity.index()] {
                left.push(Left::Scar(x.region));
            }
            lines.push(format!("{name}'s {} {how}", x.region.label()));
            false
        });
        for l in &left {
            if let Left::Scar(r) = *l {
                if !w.has_mark(r, Mark::Scar) {
                    w.marks.push((r, Mark::Scar));
                }
            }
        }
        let blood = w.blood;
        let cause = w.list.iter().filter(|x| x.bleeding).max_by_key(|x| x.severity).map(|x| (x.by, x.murder));
        let legs_changed = before != w.list.len();
        if w.list.is_empty() && w.blood >= 1.0 {
            world.hurt.remove(&id);
            if w.marks.is_empty() {
                world.remove::<Wounds>(id);
            } else if let Some(x) = world.comp_mut::<Wounds>(id) {
                *x = w;
            }
        } else if let Some(x) = world.comp_mut::<Wounds>(id) {
            *x = w;
        }
        for l in left {
            match l {
                Left::Maim(r, t) => maim_region(world, id, r, t),
                Left::Lose(r) => lose_limb(world, id, r, "a wound never seen to festered"),
                Left::Scar(_) => {}
            }
        }
        if legs_changed {
            crate::systems::assets::rekit(world, id);
        }
        for l in lines {
            world.push_event(EventKind::Fight, &[id], l);
        }
        if down.is_none() && blood <= 0.0 {
            let (by, murder) = cause.unwrap_or((None, false));
            if !go_down(world, id, by, murder) {
                world.kill_by(id, crate::components::DeathCause::Violence, by);
                world.push_event(EventKind::BledOut, &[id], format!("{} bled out from a wound", world.name_of(id)));
            }
            continue;
        }
        if down.is_some() {
            continue;
        }
        shift_lost(world, id, tod);
        untreated_note(world, id, now, &c);
    }
}

/// At a shift's first hour on a workday: a worker unfit by his wounds is
/// turned away, the shift and its pay lost (the Work goal refuses it,
/// `routine::work_refused`; off screen the wage gate).
fn shift_lost(world: &mut World, id: EntityId, tod: u16) {
    let Some(job) = world.comp::<Job>(id).cloned() else { return };
    if !job.first_hour(tod) || world.has::<crate::components::Sentence>(id) {
        return;
    }
    let key = job.shift_key_at(world.tick);
    if !crate::exec::routine::workday_of(world, id, &job, key) {
        return;
    }
    let Some(why) = unfit(world, id, &job) else { return };
    let at = job.employer.map_or_else(|| "work".to_string(), |e| world.name_of(e));
    let text = format!(
        "{} was turned away from the shift at {at} with {why}: the day's {}c lost",
        world.name_of(id),
        job.wage_per_day
    );
    world.push_event(EventKind::Fight, &[id], text);
}

/// `untreated_note_hours` after a wound that is still not seen to: the
/// body says why (it cannot pay, or no doctor is in reach).
fn untreated_note(world: &mut World, id: EntityId, now: Tick, c: &crate::config::CombatCfg) {
    let due = Tick::from(c.untreated_note_hours) * TICKS_PER_HOUR;
    let fresh = world.comp::<Wounds>(id).is_some_and(|w| {
        w.list.iter().any(|x| {
            !x.treated
                && x.severity.index() >= c.treat_from
                && now >= x.since + due
                && now < x.since + due + TICKS_PER_HOUR
        })
    });
    if !fresh {
        return;
    }
    let Some(why) = crate::systems::customers::untreated_why(world, id) else { return };
    let text = format!("{} lives with the wound: {why}", world.name_of(id));
    world.push_event(EventKind::Fight, &[id], text);
}
