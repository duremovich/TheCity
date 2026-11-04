use super::*;

// ---------------------------------------------------------------------------
// Hits
// ---------------------------------------------------------------------------

/// `contracts::set_status`: an organic Hit or Beat a person bought closed
/// `status` (Expired or Failed) unfilled. The buyer's choice.
pub fn hit_unfilled(world: &mut World, ct: &Contract, status: ContractStatus) {
    let Some(c) = cfg(world).cloned() else { return };
    if !matches!(ct.kind, ContractKind::Hit | ContractKind::Beat) {
        return;
    }
    let (Some(b), Target::Agent(t)) = (ct.buyer, ct.target) else { return };
    if ct.agent != Some(b) || !free(world, b) || !crate::systems::hunt::target_ok(world, t) {
        return;
    }
    let grudge = crate::systems::grudges::grudge_on(world, b, t).filter(|g| g.settled.is_none());
    let g = grudge.as_ref().map_or(0.5, |x| x.weight);
    let n = world.consequence.reposts.iter().find(|&&(x, y, _)| x == b && y == t).map_or(0, |x| x.2);
    let price = ((ct.price as f32 * c.hit_repost_mult).ceil() as i64).max(ct.price + 1);
    let deposit = crate::systems::hitmarket::deposit_of(world, price);
    let repost_ok = n < c.hit_repost_max && world.purse(Some(b)) >= deposit && crate::systems::hitmarket::room(world);
    let self_ok = grudge.is_some()
        && g >= c.hit_self_min
        && crate::systems::hunt::hunter_ok(world, b)
        && !world.hunts.contains_key(&b)
        && world.hunts.len() < world.config.hunt.max_hunts;
    let tr = traits(world, b);
    let weights = [
        if repost_ok { c.hit_w_repost * (0.5 + g) } else { 0.0 },
        if self_ok { c.hit_w_self * tr.courage * g } else { 0.0 },
        c.hit_w_giveup * (1.5 - g).max(0.0),
    ];
    let (bn, tn) = (world.name_of(b), world.name_of(t));
    let kind = ct.kind.label();
    let lapse = if status == ContractStatus::Failed { "failed" } else { "found no taker" };
    match pick(&weights, hash(world, P_HIT, b, ct.id)) {
        Some(0) => {
            let broker = ct
                .broker
                .filter(|&f| crate::systems::contracts::fixer_open(world, f))
                .or_else(|| crate::systems::contracts::nearest_fixer(world, b));
            let posting = Posting {
                buyer: Some(b),
                agent: Some(b),
                kind: ct.kind,
                target: ct.target,
                broker,
                deadline_days: world.config.contracts.deadline_days.get(ct.kind),
                origin: ct.origin,
                price: Some(price),
            };
            let extra = PostExtra {
                deposit: (deposit < price).then_some(deposit),
                motive: ct.motive,
                why: Some(format!("raised from {}c", ct.price)),
            };
            if crate::systems::contracts::post_with(world, posting, extra).is_ok() {
                match world.consequence.reposts.iter_mut().find(|x| x.0 == b && x.1 == t) {
                    Some(x) => x.2 += 1,
                    None => world.consequence.reposts.push((b, t, 1)),
                }
                let at = broker.map(|f| format!(" to {}", world.name_of(f))).unwrap_or_default();
                let text = format!(
                    "{bn} went back{at} and raised the price on {tn}: the {kind} {lapse} at {}c, now {price}c",
                    ct.price
                );
                let actors = [b, t, broker.unwrap_or(EntityId::NONE)];
                world.push_event(EventKind::HitEscalated, &actors, text);
                ownership::jobs2_row(world).hit_reposted += 1;
                return;
            }
            give_up(world, &c, b, t, kind, lapse, ct.price);
        }
        Some(1) => go_himself(world, b, t, g, kind, lapse),
        _ => give_up(world, &c, b, t, kind, lapse, ct.price),
    }
}

fn give_up(world: &mut World, c: &ConsequenceCfg, b: EntityId, t: EntityId, kind: &str, lapse: &str, price: i64) {
    let mut cooled = None;
    if let Some(g) = world.comp_mut::<crate::word::Grudges>(b) {
        if let Some(x) = g.list.iter_mut().find(|x| x.target == t && x.settled.is_none()) {
            let was = x.weight;
            x.weight *= c.hit_giveup_decay;
            cooled = Some((was, x.weight));
        }
    }
    let (bn, tn) = (world.name_of(b), world.name_of(t));
    let tail = cooled.map_or_else(String::new, |(a, z)| format!("; the grudge cooled {a:.2} -> {z:.2}"));
    let text = format!("{bn} let it go: the {kind} on {tn} {lapse} at {price}c{tail}");
    world.push_event(EventKind::HitEscalated, &[b, t], text);
    ownership::jobs2_row(world).hit_giveup += 1;
}

fn go_himself(world: &mut World, b: EntityId, t: EntityId, weight: f32, kind: &str, lapse: &str) {
    if world.comp::<Brain>(b).is_some_and(|x| x.lod == Lod::Statistical) {
        crate::systems::lod::set_lod(world, b, Lod::Coarse);
    }
    let chain = crate::systems::grudges::grudge_on(world, b, t).map_or(0, |g| g.chain);
    let gap = crate::systems::hunt::might_gap(world, b, t);
    let now = world.tick;
    world.hunts.insert(
        b,
        HuntState {
            target: t,
            grudge_target: t,
            chain,
            since: now,
            phase: HuntPhase::Ask,
            venue: None,
            intel: None,
            stakeout_until: None,
            deceived: false,
            why: HuntWhy::Goal,
            liar: None,
            weight,
            gap,
        },
    );
    if let Some(x) = world.comp_mut::<Brain>(b) {
        x.cooldowns.remove(&crate::components::GoalKind::Hunt);
    }
    world.stats.current.word.hunts += 1;
    let (bn, tn) = (world.name_of(b), world.name_of(t));
    let (he, him, _) = pro(world, b);
    let text = format!("{bn} stopped waiting: the {kind} on {tn} {lapse}, so {he} went after {tn} {him}self");
    world.push_event(EventKind::HitEscalated, &[b, t], text);
    world.push_event(EventKind::HuntStarted, &[b, t], format!("{bn} went looking for {tn}"));
    crate::systems::hunt::reindex(world);
    ownership::jobs2_row(world).hit_self += 1;
}

// ---------------------------------------------------------------------------
// Jail
// ---------------------------------------------------------------------------

/// `law::sentence`, before the job is touched: the run begins; the
/// prisoner learns who reported him (a civilian: a Betrayed grudge).
pub fn on_sentenced(world: &mut World, who: EntityId, crime: Crime) {
    let Some(c) = cfg(world).cloned() else { return };
    if !person(world, who) {
        return;
    }
    let reporter = world
        .crime_reports()
        .iter()
        .rev()
        .filter(|r| r.suspect == who)
        .filter_map(|r| r.witness)
        .find(|&w| w != who && person(world, w) && !crate::systems::law::is_guard(world, w));
    let job = world.comp::<Job>(who).filter(|j| j.employer.is_some()).map(|j| (j.role.label().to_string(), j.employer));
    let day = world.day();
    let home = world.comp::<crate::components::Household>(who).and_then(|h| h.home);
    let gang = world.gang_of(who);
    world.consequence.inside.retain(|(p, _)| *p != who);
    world.consequence.inside.push((who, Inside { since: day, job, reporter, home, gang, ..Default::default() }));
    if let Some(r) = reporter {
        crate::systems::grudges::add(world, who, r, GrudgeCause::Betrayed, c.snitch_grudge, 0);
        let him = pro(world, who).1;
        let text = format!(
            "{} learned who put {him} inside: {} reported {him} for the {}",
            world.name_of(who),
            world.name_of(r),
            crime.label()
        );
        world.push_event(EventKind::Cells, &[who, r], text);
        ownership::jobs2_row(world).snitch_grudges += 1;
    }
}

/// `law::release`: the life he comes out to.
pub fn on_released(world: &mut World, who: EntityId) {
    if cfg(world).is_none() {
        return;
    }
    let Some(i) = world.consequence.inside.iter().position(|(p, _)| *p == who) else { return };
    let (_, x) = world.consequence.inside.remove(i);
    let mut parts: Vec<String> = Vec::new();
    // Round 8 X6 (`OPEN-217`): what waits outside, kept or not (the post,
    // the Home, the gang); `changed` the old line's count.
    let mut kept: Vec<String> = Vec::new();
    // "still his" / "still hers": the possessive standing alone.
    let his = if pro(world, who).2 == "her" { "hers" } else { "his" };
    if let Some((role, employer)) = &x.job {
        let still = world.comp::<Job>(who).is_some_and(|j| j.employer == *employer);
        let at = employer.map(|e| format!(" at {}", world.name_of(e))).unwrap_or_default();
        if still {
            kept.push(format!("the post as {role}{at} still {his}"));
        } else {
            parts.push(format!("the post as {role}{at} gone"));
        }
    }
    let home_now = world.comp::<crate::components::Household>(who).and_then(|h| h.home);
    match (x.home, home_now) {
        (Some(h), Some(n)) if h == n => kept.push(format!("the Home at {} still {his}", world.name_of(h))),
        (Some(h), Some(n)) => {
            parts.push(format!("the Home at {} gone, {} {his} now", world.name_of(h), world.name_of(n)))
        }
        (Some(h), None) => parts.push(format!("the Home at {} gone: nowhere to sleep", world.name_of(h))),
        (None, Some(n)) => parts.push(format!("a Home at {} now", world.name_of(n))),
        (None, None) => kept.push("no Home to go to".to_string()),
    }
    if let Some(g) = x.gang.filter(|&g| Some(g) != x.joined) {
        if world.gang_of(who) == Some(g) {
            kept.push(format!("{} still {his}", gang_name(world, g)));
        } else if world.comp::<Gang>(g).is_some() {
            parts.push(format!("{} done with {}", gang_name(world, g), pro(world, who).1));
        } else {
            parts.push(format!("{} gone", gang_name(world, g)));
        }
    }
    for &(l, n) in &x.owed {
        let left = crate::systems::underwater::owed(world, who, l);
        if left > 0 {
            parts.push(format!("{left}c owed to {} (of {n}c borrowed inside)", world.name_of(l)));
        }
    }
    if let Some(r) = x.reporter.filter(|&r| crate::systems::grudges::holds(world, who, r, 0.0)) {
        parts.push(format!("a grudge on {}, who reported {}", world.name_of(r), pro(world, who).1));
    }
    if let Some(g) = x.joined.filter(|&g| world.gang_of(who) == Some(g)) {
        parts.push(format!("one of {} now", gang_name(world, g)));
    }
    if let Some(f) = x.fixer {
        parts.push(format!("a name at {}", world.name_of(f)));
    }
    if x.fights > 0 {
        let n = x.fights;
        parts.push(format!("{n} fight{} in the cells behind {}", if n == 1 { "" } else { "s" }, pro(world, who).1));
    }
    let changed = !parts.is_empty();
    let text = if changed {
        let and = if kept.is_empty() { String::new() } else { format!("; {}", kept.join("; ")) };
        format!("{} came out to a changed life: {}{and}", world.name_of(who), parts.join("; "))
    } else {
        format!("{} came out to the life {} left: {}", world.name_of(who), pro(world, who).0, kept.join("; "))
    };
    world.push_event(EventKind::Cells, &[who], text);
    if changed {
        ownership::jobs2_row(world).released_changed += 1;
    }
}

/// A Fixer `m` knows: one he owns, works at, or is a regular of.
pub(super) fn fixer_known(world: &World, m: EntityId) -> Option<EntityId> {
    let mine = world.comp::<Job>(m).and_then(|j| j.employer);
    world.buildings_of_kind(BuildingKind::Fixer).iter().copied().find(|&f| {
        world.owner_of(f) == Some(m)
            || mine == Some(f)
            || world.comp::<Broker>(f).is_some_and(|k| k.regulars.contains_key(&m))
    })
}

pub(super) fn regular_anywhere(world: &World, p: EntityId) -> bool {
    world
        .buildings_of_kind(BuildingKind::Fixer)
        .iter()
        .any(|&f| world.comp::<Broker>(f).is_some_and(|k| k.regulars.contains_key(&p)))
}
