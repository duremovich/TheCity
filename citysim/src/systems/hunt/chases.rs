use super::*;

/// M16a (plan C14): the intel state a chase step reads, from the agent's
/// Hunt (`World::hunts`) or else its contract run (`World::contract_runs`):
/// a copy, written back by [`set_chase`]. With no contract run every
/// reader sees exactly the Hunt's state.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Chase {
    pub target: EntityId,
    pub phase: HuntPhase,
    pub venue: Option<EntityId>,
    pub intel: Option<Intel>,
    pub stakeout_until: Option<Tick>,
    pub deceived: bool,
    pub liar: Option<EntityId>,
}

/// The agent's chase: its Hunt's, else its contract run's.
pub fn chase(world: &World, id: EntityId) -> Option<Chase> {
    if let Some(s) = world.hunts.get(&id) {
        return Some(Chase {
            target: s.target,
            phase: s.phase,
            venue: s.venue,
            intel: s.intel,
            stakeout_until: s.stakeout_until,
            deceived: s.deceived,
            liar: s.liar,
        });
    }
    if world.contract_runs.is_empty() {
        return None;
    }
    world.contract_runs.get(&id).map(|r| Chase {
        target: r.target,
        phase: r.phase,
        venue: r.venue,
        intel: r.intel,
        stakeout_until: r.stakeout_until,
        deceived: r.deceived,
        liar: r.liar,
    })
}

/// Write a chase back to whichever store holds the agent (the target is
/// never rewritten).
pub fn set_chase(world: &mut World, id: EntityId, c: Chase) {
    if let Some(s) = world.hunts.get_mut(&id) {
        s.phase = c.phase;
        s.venue = c.venue;
        s.intel = c.intel;
        s.stakeout_until = c.stakeout_until;
        s.deceived = c.deceived;
        s.liar = c.liar;
        return;
    }
    if let Some(r) = world.contract_runs.get_mut(&id) {
        r.phase = c.phase;
        r.venue = c.venue;
        r.intel = c.intel;
        r.stakeout_until = c.stakeout_until;
        r.deceived = c.deceived;
        r.liar = c.liar;
    }
}

/// W20: take up a Hunt on the heaviest eligible grudge (a god Hunt skips
/// the cap). `HuntStarted`, `stats.hunts`, the chain statistic.
pub fn adopt(world: &mut World, id: EntityId, why: HuntWhy) -> bool {
    if world.hunts.contains_key(&id) {
        return true;
    }
    if why != HuntWhy::God && world.hunts.len() >= world.config.hunt.max_hunts {
        return false;
    }
    let Some((target, weight, chain)) = heaviest_eligible(world, id) else { return false };
    // The Hit market (D8, phase 4 fix): a buyer does not hunt the head it is
    // paying for (a god's Hunt still goes ahead and ends the want).
    if why != HuntWhy::God && crate::systems::hitmarket::paying_for(world, id, target) {
        return false;
    }
    // M16a (plan C20): a holder too weak to win who can pay hires instead
    // (a Goal or Stat Hunt; never a god's). Round 9 Z1: one who cannot pay
    // hands the grudge on (`hand_off`).
    if why != HuntWhy::God && hand_off(world, id, target, weight, chain) {
        return false;
    }
    let now = world.tick;
    let gap = might_gap(world, id, target);
    world.hunts.insert(
        id,
        HuntState {
            target,
            grudge_target: target,
            chain,
            since: now,
            phase: HuntPhase::Ask,
            venue: None,
            intel: None,
            stakeout_until: None,
            deceived: false,
            why,
            liar: None,
            weight,
            gap,
        },
    );
    let w = &mut world.stats.current.word;
    w.hunts += 1;
    // Plan deviation: the CSV's `chain_max` is a chain's length (the
    // grudge's chain + 1), so "a revenge chain of length 2" reads 2.
    w.chain_max = w.chain_max.max(u32::from(chain) + 1);
    let text = format!("{} went looking for {}", world.name_of(id), world.name_of(target));
    world.push_event(EventKind::HuntStarted, &[id, target], text);
    heard(world, id, target);
    true
}

/// Round 9 Z1: a holder who would hire rather than go (too weak or too
/// cowardly, `hitmarket::hires`) hands the grudge on, each with a line: a
/// lawful one leaves a wanted man to the law (`hitmarket::waits_for_law`);
/// one who can pay hires through a fixer (`contracts::try_hire`, the posting
/// path; the take is the market's); one who cannot asks the strongest of
/// his own people who will go (`hand_on`), else tells the law of a killing
/// (`tell_law`). True when handed (no Hunt taken up). The under-18 rule:
/// a minor neither holds a Hunt nor takes one (`hunter_ok`).
fn hand_off(world: &mut World, id: EntityId, target: EntityId, weight: f32, chain: u8) -> bool {
    let hires = crate::systems::hitmarket::hires(world, id, target);
    let law_wait = crate::systems::hitmarket::waits_for_law(world, id, target);
    if hires && law_wait && !noted(world, id, target, NOTED_LAW) {
        let what = crate::systems::law::wanted_for(world, target)
            .map_or_else(|| "the law has the name".to_string(), |c| format!("wanted for {}", c.label()));
        let text = format!(
            "{} leaves {} to the law{}: {what}",
            world.name_of(id),
            world.name_of(target),
            over(world, id, target)
        );
        line(world, &[id], text);
        mark(world, id, target, NOTED_LAW);
        world.stats.current.word.hunts_handed += 1;
    }
    let wanting = world.wants.contains_key(&id);
    if crate::systems::contracts::try_hire(world, id, target, weight) {
        if !law_wait {
            world.stats.current.word.hunts_handed += 1;
            // The walk to the fixer is the errand (H9); a direct post has
            // its own line.
            if let Some(f) = world.wants.get(&id).filter(|w| !wanting && w.target == target).map(|w| w.fixer) {
                let text = format!(
                    "{}, {}, will pay to have it done{}: off to {}",
                    world.name_of(id),
                    unfit(world, id, target),
                    over(world, id, target),
                    world.name_of(f)
                );
                line(world, &[id, f], text);
            }
        }
        return true;
    }
    if !hires {
        return false;
    }
    hand_on(world, id, target, weight, chain) || tell_law(world, id, target)
}

/// Z1: why `id` does not go himself, in words.
fn unfit(world: &World, id: EntityId, target: EntityId) -> String {
    let old = u64::from(world.config.ages.frail_from_years) * crate::time::DAYS_PER_YEAR;
    if world.comp::<crate::components::Identity>(id).is_some_and(|i| u64::from(i.age_days) >= old) {
        "too old to fight".to_string()
    } else if world.comp::<Personality>(id).is_some_and(|p| p.courage < world.config.hitmarket.hire_courage) {
        "without the nerve".to_string()
    } else {
        format!("no match for {}", pro(world, target).1)
    }
}

/// Z1: the Hunt held off `days` (a grudge handed on is not hunted twice).
fn hold_off(world: &mut World, id: EntityId, days: u32) {
    let until = world.tick + Tick::from(days) * TICKS_PER_DAY;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        let t = b.cooldowns.entry(GoalKind::Hunt).or_insert(until);
        *t = (*t).max(until);
    }
}

/// Z1: a grudge over a killing handed to the strongest of `id`'s own who
/// will go himself (kin, a friend, a gang mate: a free adult not hunting,
/// not of the target's gang, who would not hire): an `Inherited` grudge at
/// `hand_w` of the weight, the line, and `id`'s Hunt held off `hunt_days`.
fn hand_on(world: &mut World, id: EntityId, target: EntityId, weight: f32, chain: u8) -> bool {
    let share = world.config.hunt.hand_w;
    if share <= 0.0 {
        return false;
    }
    if !crate::systems::grudges::grudge_on(world, id, target).is_some_and(|g| blood(g.cause)) {
        return false;
    }
    let w = (weight * share).min(1.0);
    if w < min_for(world, GrudgeCause::Inherited(id)) {
        return false;
    }
    let mut cands: Vec<EntityId> = world
        .neighbours
        .get(&id)
        .map(|n| {
            n.iter()
                .copied()
                .filter(|&o| {
                    world.edge(id, o).is_some_and(|e| {
                        matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family | RelKind::Friend)
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(g) = world.gang_of(id).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        cands.extend(g.members.iter().copied());
    }
    let Some((_, o)) = cands
        .into_iter()
        .filter(|&o| o != id && o != target && hunter_ok(world, o) && !world.hunts.contains_key(&o))
        .filter(|&o| takes_against(world, o, target))
        .filter(|&o| !crate::systems::hitmarket::hires(world, o, target))
        .map(|o| (might_gap(world, o, target), o))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
    else {
        return false;
    };
    let text = format!(
        "{}, {}, asked {} to go after {}{}",
        world.name_of(id),
        unfit(world, id, target),
        world.name_of(o),
        world.name_of(target),
        over(world, id, target)
    );
    line(world, &[id, o], text);
    crate::systems::grudges::add(world, o, target, GrudgeCause::Inherited(id), w, chain);
    world.stats.current.word.hunts_handed += 1;
    let days = world.config.hunt.hunt_days;
    hold_off(world, id, days);
    true
}

/// Z1: a killing told to the law by a holder who tells it
/// (`law::tells_the_law`) when the killer is not wanted already: a report
/// naming him, its line, the Hunt held off `hunt_cooldown_days`.
fn tell_law(world: &mut World, id: EntityId, target: EntityId) -> bool {
    if world.config.hunt.hand_w <= 0.0 || noted(world, id, target, NOTED_LAW) {
        return false;
    }
    let Some(dead) = crate::systems::grudges::grudge_on(world, id, target).and_then(|g| killing(g.cause)) else {
        return false;
    };
    if crate::systems::law::wanted(world, target) || !crate::systems::law::tells_the_law(world, id, target) {
        return false;
    }
    let text = format!(
        "{}, {}, told the law {} killed {}",
        world.name_of(id),
        unfit(world, id, target),
        world.name_of(target),
        if world.has::<crate::components::Identity>(dead) {
            world.name_of(dead)
        } else {
            format!("one of {} own", crate::systems::grief::his(world, id))
        }
    );
    let actors = [id, target];
    if !crate::systems::law::file_report_as(
        world,
        crate::components::Crime::Murder,
        target,
        Some(id),
        Some((text, &actors)),
    ) {
        return false;
    }
    mark(world, id, target, NOTED_LAW);
    world.stats.current.word.hunts_handed += 1;
    let days = world.config.hunt.hunt_cooldown_days;
    hold_off(world, id, days);
    true
}

/// Z1: the hunted gets word (`heard_p`, a hash of the pair): his safety
/// falls, he remembers the threat, and he acts with a line: one brave and
/// lawless enough swears to get the hunter first (a grudge at
/// `heard_counter_w`), anyone else lies low at home `heard_low_days`
/// (`grief::lie_low`). Never on a minor (`target_ok`, `hunter_ok`).
pub(super) fn heard(world: &mut World, hunter: EntityId, target: EntityId) {
    const PURPOSE_HUNTED: u64 = 0x2a_4e_7d_31;
    let c = world.config.hunt.clone();
    if c.heard_p <= 0.0 || !target_ok(world, target) || !world.has::<crate::components::Identity>(target) {
        return;
    }
    let roll = crate::systems::econ::hash_unit(
        world.seed(),
        PURPOSE_HUNTED,
        u64::from(hunter.index),
        (u64::from(target.index) << 20) ^ time::day(world.tick),
    );
    if roll >= c.heard_p {
        return;
    }
    if let Some(n) = world.comp_mut::<crate::components::Needs>(target) {
        n.safety = (n.safety - c.heard_safety).max(0.0);
    }
    world.remember(target, MemoryKind::Threatened, Some(hunter), 0.6, -0.5, false);
    world.stats.current.word.hunted_heard += 1;
    let (tn, hn) = (world.name_of(target), world.name_of(hunter));
    let (he, him) = pro(world, target);
    let why = over(world, hunter, target);
    let bold = world
        .comp::<Personality>(target)
        .is_some_and(|p| p.courage >= c.heard_counter_courage && p.lawfulness <= c.heard_counter_lawful);
    if bold && c.heard_counter_w > 0.0 && hunter_ok(world, hunter) {
        crate::systems::grudges::add(world, target, hunter, GrudgeCause::Assaulted, c.heard_counter_w, 0);
        line(world, &[target], format!("{tn} got word {hn} is looking for {him}{why}, and swore to get {hn} first"));
    } else if c.heard_low_days > 0 {
        let until = (world.tick / TICKS_PER_DAY + 1 + u64::from(c.heard_low_days)) * TICKS_PER_DAY;
        let text =
            format!("{tn} got word {hn} is looking for {him}{why}: {he} lies low at home {} days", c.heard_low_days);
        line(world, &[target], text);
        crate::systems::grief::lie_low(world, target, until, format!("word {hn} is looking for {him}"));
    } else {
        line(world, &[target], format!("{tn} got word {hn} is looking for {him}{why}"));
    }
}
