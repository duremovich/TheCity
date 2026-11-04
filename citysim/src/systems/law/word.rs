use super::*;

/// Round 6 V3 (`OPEN-238`): would `w` take what it saw of `suspect` to the
/// law (the ReportCrime goal's filters, for a word on the street): a waking,
/// free adult on its feet and wanted for nothing, `[crime.case]
/// tell_lawful_min` lawful, no gang
/// member (unless betraying), not keeping it to sell, and never on a minor
/// (CLAUDE.md, the under-18 rule: no warrant, so no arrest force, on a child).
pub fn tells_the_law(world: &World, w: EntityId, suspect: EntityId) -> bool {
    let min = world.config.crime.case.tell_lawful_min;
    living(world, w)
        && living(world, suspect)
        && w != suspect
        && !wanted(world, w)
        && crate::systems::demography::is_adult(world, w)
        && !crate::systems::ages::is_protected(world, suspect)
        && !world.has::<Sentence>(w)
        && !crate::systems::wounded::is_down(world, w)
        && world.comp::<crate::components::Personality>(w).is_some_and(|p| p.lawfulness >= min)
        && (world.gang_of(w).is_none() || world.comp::<Brain>(w).is_some_and(|b| b.betraying))
        && world
            .comp::<Brain>(w)
            .is_some_and(|b| !matches!(b.exec, crate::exec::ExecState::Use { kind: ActionKind::Sleep, .. }))
        && !crate::systems::consequence::keeps_quiet(world, w, suspect)
        && fears(world, w, suspect).is_none()
}

/// Round 7 W2 (`OPEN-238`): `suspect`'s gang holds the block `w` lives on
/// and `w` is under `[crime.case] fear_courage_max` brave: the gang, feared.
fn fears(world: &World, w: EntityId, suspect: EntityId) -> Option<EntityId> {
    let g = world.gang_of(suspect)?;
    if courage(world, w) >= world.config.crime.case.fear_courage_max || world.gang_of(w) == Some(g) {
        return None;
    }
    (crate::systems::turf::home_holder(world, w) == Some(g)).then_some(g)
}

/// Round 7 W2 (`OPEN-238`): why `w` keeps what he knows of `suspect` from
/// the law (`tells_the_law` false), for his line: his gang, the suspect's
/// gang on his block, the sale, his own warrant, his nature; `None` when he
/// would tell.
pub fn keeps_from_law(world: &World, w: EntityId, suspect: EntityId) -> Option<String> {
    if tells_the_law(world, w, suspect) {
        return None;
    }
    let (he, him, his) = crate::systems::consequence::pro(world, w);
    // The under-18 rule: a child is never named a culprit, not even here.
    if crate::systems::ages::is_protected(world, suspect) {
        return Some(format!("{he} says nothing"));
    }
    let gname = |g: EntityId| world.comp::<crate::components::Gang>(g).map_or_else(String::new, |x| x.name.clone());
    let betraying = world.comp::<Brain>(w).is_some_and(|b| b.betraying);
    if let Some(g) = world.gang_of(w).filter(|_| !betraying) {
        return Some(if world.gang_of(suspect) == Some(g) {
            format!("{} runs with {}, {his} own crew", world.name_of(suspect), gname(g))
        } else {
            format!("{he} runs with {}, and the law is not their way", gname(g))
        });
    }
    if let Some(g) = fears(world, w, suspect) {
        return Some(format!("afraid: {} hold the block {he} lives on", gname(g)));
    }
    if crate::systems::consequence::keeps_quiet(world, w, suspect) {
        return Some(format!("it is worth something to {him}"));
    }
    if wanted(world, w) {
        return Some(format!("{he} is wanted {him}self"));
    }
    let min = world.config.crime.case.tell_lawful_min;
    if world.comp::<crate::components::Personality>(w).is_some_and(|p| p.lawfulness < min) {
        return Some(format!("{he} is not one to go to the law"));
    }
    Some(format!("{he} says nothing"))
}

/// Round 6 V3 (`OPEN-238`): the nearest on-duty city guard outdoors within
/// `[crime.case] hue_reach` of `tile` (Chebyshev; ties the lower id), not on
/// `suspect`'s take.
fn guard_in_earshot(world: &World, tile: TilePos, suspect: EntityId) -> Option<EntityId> {
    let reach = world.config.crime.case.hue_reach;
    world
        .guards()
        .iter()
        .copied()
        .filter(|&g| g != suspect && on_duty(world, g) && is_city_guard(world, g) && !world.has::<Sentence>(g))
        .filter_map(|g| {
            let p = world.comp::<Position>(g)?;
            let d = chebyshev(p.tile, tile);
            (d <= reach && outdoors(world, p.tile, p.building)).then_some((d, g))
        })
        .filter(|&(_, g)| !crate::systems::contracts::on_take_of(world, g, suspect))
        .min()
        .map(|(_, g)| g)
}

/// Round 6 V3 (`OPEN-238`): a victim on the street shouts, and an on-duty
/// guard in earshot takes the report on the spot (the witness the victim,
/// the suspect seen there). V14 `vendor_9648 d23`: ten thieves on one till,
/// a SawCrime each, and no report (the Hall out of `report_ticks` reach).
pub fn hue_and_cry(world: &mut World, victim: EntityId, actor: EntityId, crime: Crime, tile: TilePos) {
    if is_petty(crime) || !tells_the_law(world, victim, actor) {
        return;
    }
    let Some((vt, vb)) = world.comp::<Position>(victim).map(|p| (p.tile, p.building)) else { return };
    if !outdoors(world, vt, vb) {
        return;
    }
    let Some(g) = guard_in_earshot(world, vt, actor) else { return };
    let (vn, gn, an) = (world.name_of(victim), world.name_of(g), world.name_of(actor));
    let text = format!("{vn} cried out and {gn} took the report: {an} for {}", crime.label());
    if file_report_as(world, crime, actor, Some(victim), Some((text, &[victim, g]))) {
        world.set_last_seen(actor, (tile, world.tick));
    }
}

/// Round 6 V3 (`OPEN-238`), hourly: an on-duty city guard on the street
/// meets the people around him (`[crime.case] hue_reach`); one holding a
/// first-hand sighting from the last `tell_days` (as its victim, or of a
/// Murder) of a suspect not reported since tells him, and he takes the
/// report. The street's way to the law where the one Hall is out of reach.
pub(super) fn tell_the_beat(world: &mut World) {
    let cfg = world.config.crime.case.clone();
    let since = world.tick.saturating_sub(cfg.tell_days * TICKS_PER_DAY);
    let guards: Vec<EntityId> = world
        .guards()
        .iter()
        .copied()
        .filter(|&g| on_duty(world, g) && is_city_guard(world, g) && !world.has::<Sentence>(g))
        .collect();
    // One tell a person a pass (V3 shadow: one victim told five guards five
    // suspects in one minute).
    let mut tellers: BTreeSet<EntityId> = BTreeSet::new();
    for g in guards {
        let Some((gt, gb)) = world.comp::<Position>(g).map(|p| (p.tile, p.building)) else { continue };
        if !outdoors(world, gt, gb) {
            continue;
        }
        let near: Vec<EntityId> = world.agents_near(gt, cfg.hue_reach, None);
        for w in near {
            if w == g
                || tellers.contains(&w)
                || world.comp::<Brain>(w).is_none_or(|b| b.lod == crate::components::Lod::Statistical)
            {
                continue;
            }
            let Some((wt, wb)) = world.comp::<Position>(w).map(|p| (p.tile, p.building)) else { continue };
            if !outdoors(world, wt, wb) || chebyshev(wt, gt) > cfg.hue_reach {
                continue;
            }
            let Some(mem) = world.comp::<crate::components::Memory>(w) else { continue };
            let told = mem
                .entries
                .iter()
                .filter(|e| e.kind == MemoryKind::SawCrime && !e.second_hand && e.tick >= since)
                .filter_map(|e| Some((e.subject?, e.crime?, e.object, e.tick, e.salience)))
                .filter(|&(_, c, o, _, _)| !is_petty(c) && (o == Some(w) || c == Crime::Murder))
                .filter(|&(s, _, _, t, _)| !reported_since(world, s, t))
                .max_by(|a, b| a.4.total_cmp(&b.4))
                .map(|(s, c, _, _, _)| (s, c));
            let Some((s, crime)) = told else { continue };
            if !tells_the_law(world, w, s) || crate::systems::contracts::on_take_of(world, g, s) {
                continue;
            }
            let (wn, gn, sn) = (world.name_of(w), world.name_of(g), world.name_of(s));
            let text = format!("{wn} told {gn} on the street: {sn}, for {}", crime.label());
            tellers.insert(w);
            file_report_as(world, crime, s, Some(w), Some((text, &[w, g])));
        }
    }
}

/// Round 8 X1 (`OPEN-246`): the law's desks open now: the Halls by day
/// (Morning, Work, Evening: ReportCrime's hours), the Precinct and the
/// Stations around the clock (a desk sergeant at each).
fn desks_open(world: &World) -> Vec<EntityId> {
    use crate::time::DayPhase;
    let mut v: Vec<EntityId> = Vec::new();
    if matches!(world.phase(), DayPhase::Morning | DayPhase::Work | DayPhase::Evening) {
        v.extend(world.buildings_of_kind(BuildingKind::Hall).iter().copied());
    }
    v.extend(world.buildings_of_kind(BuildingKind::Jail).iter().copied());
    v.extend(world.buildings_of_kind(BuildingKind::Station).iter().copied());
    v.retain(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished));
    v
}

/// Round 8 X1: the nearest open desk by the trip from where `id` sets out
/// (`distance::trip_to`; ties the lower id), with that trip; `None` past
/// `[crime.case] word_reach_ticks`.
pub fn law_desk(world: &World, id: EntityId) -> Option<(EntityId, Tick)> {
    let (t, b) = desks_open(world)
        .into_iter()
        .filter_map(|b| crate::systems::distance::trip_to(world, id, b).map(|t| (t, b)))
        .min()?;
    (t <= world.config.crime.case.word_reach_ticks).then_some((b, t))
}

/// Round 8 X1: the promise `w` carries: the first standing word owed
/// (`consequence::owed_word`) on a suspect he would still name now.
pub fn word_owed(world: &World, w: EntityId) -> Option<EntityId> {
    let now = world.tick;
    world
        .consequence
        .owed_word
        .iter()
        .find(|&&(v, s, until)| v == w && until > now && !world.has::<Sentence>(s) && tells_the_law(world, w, s))
        .map(|&(_, s, _)| s)
}

/// Round 8 X1: `w` gave his word to tell the law of `s` and it stands
/// (the Fight goal leaves him to the law).
pub fn owes_word_on(world: &World, w: EntityId, s: EntityId) -> bool {
    let now = world.tick;
    world.consequence.owed_word.iter().any(|&(v, x, until)| v == w && x == s && until > now)
}

/// Round 8 X1: the promise's plan: the nearest open desk, the report there
/// (`ReportCrime` on the suspect: `exec::actions::report_crime` files it).
pub fn word_plan(world: &World, w: EntityId) -> Option<crate::goap::Plan> {
    use crate::components::{ActionInstance, GoalKind};
    let s = word_owed(world, w)?;
    let (desk, _) = law_desk(world, w)?;
    let steps = vec![
        ActionInstance { action: ActionKind::GoTo(crate::goap::LocationKey::Hall), target: Some(desk), tile: None },
        ActionInstance { action: ActionKind::ReportCrime, target: Some(s), tile: None },
    ];
    Some(crate::goap::Plan { goal: GoalKind::ReportCrime, target: Some(s), steps, started_tick: world.tick })
}

/// Round 8 X1: `w` gives the law his word on `s` at `desk` (the ReportCrime
/// step at the desk, or `off`: a Statistical promiser's hour at it): the
/// report in his name (his freshest first-hand sighting of `s`, else the
/// shakedown). A refreshed warrant files no new report: the word is given
/// all the same (`file_report_as` reads `consequence::word_given` first).
pub fn told_at_desk(world: &mut World, w: EntityId, s: EntityId, desk: Option<EntityId>, off: bool) {
    let crime = world
        .comp::<crate::components::Memory>(w)
        .and_then(|m| {
            m.entries
                .iter()
                .filter(|e| e.kind == MemoryKind::SawCrime && !e.second_hand && e.subject == Some(s))
                .max_by_key(|e| e.tick)
                .and_then(|e| e.crime)
        })
        .unwrap_or(Crime::Extortion);
    let at = desk.map_or_else(|| "the law".to_string(), |d| world.name_of(d));
    let off = if off { " (off screen)" } else { "" };
    let text =
        format!("{} went to {at} and told the law: {}, for {}{off}", world.name_of(w), world.name_of(s), crime.label());
    let mut actors = vec![w];
    actors.extend(desk);
    file_report_as(world, crime, s, Some(w), Some((text, &actors)));
    if let Some(p) = world.comp_mut::<Personality>(w) {
        p.drift(Drift::ReportedCrime);
    }
}

/// Round 8 X1: a Statistical promiser is up and off his shift this hour
/// (`presence::stat_home`'s schedule: not the night 00:00-06:00, not on a
/// shift he works).
fn stat_free(world: &World, w: EntityId) -> bool {
    if u64::from(world.tick_of_day()) < 6 * crate::time::TICKS_PER_HOUR {
        return false;
    }
    world.comp::<Job>(w).is_none_or(|j| {
        !(j.employer.is_some()
            && j.on_shift(world.tick_of_day())
            && crate::exec::routine::workday_of(world, w, j, j.shift_key_at(world.tick)))
    })
}

/// Pressure pass round 9, Y2 (`OPEN-250`): the tick `w`'s freshest
/// first-hand sighting of `s` goes stale to the beat (`tell_the_beat`'s
/// `[crime.case] tell_days` window); `None` with no such sighting.
fn beat_until(world: &World, w: EntityId, s: EntityId) -> Option<Tick> {
    let days = world.config.crime.case.tell_days;
    if days == 0 {
        return None;
    }
    let seen = world
        .comp::<crate::components::Memory>(w)?
        .entries
        .iter()
        .filter(|e| e.kind == MemoryKind::SawCrime && !e.second_hand && e.subject == Some(s))
        .map(|e| e.tick)
        .max()?;
    Some(seen + days * TICKS_PER_DAY)
}

/// Round 8 X1: `w` drops the word owed on `s`, the reason on his line.
pub fn drop_word(world: &mut World, w: EntityId, s: EntityId, why: &str) {
    let before = world.consequence.owed_word.len();
    world.consequence.owed_word.retain(|&(v, x, _)| !(v == w && x == s));
    if world.consequence.owed_word.len() == before {
        return;
    }
    crate::systems::ownership::jobs2_row(world).word_dropped += 1;
    let text = format!("{} never told the law of {}: {why}", world.name_of(w), world.name_of(s));
    world.push_event(EventKind::WitnessReacted, &[w, s], text);
}

/// Round 8 X1 (`OPEN-246`), hourly: each standing promise is kept (the
/// ReportCrime goal, the tell, the hue), waits (asleep, down) or is dropped
/// with the reason: the man taken in, the suspect held or dead, the teller
/// taken in himself, his gang, fear, his own warrant, the hours gone by with
/// no desk in reach. V16: 5 of 7 "means to tell" in the diaries never did.
pub(super) fn keep_word(world: &mut World) {
    if world.consequence.owed_word.is_empty() {
        return;
    }
    let now = world.tick;
    let words = world.consequence.owed_word.clone();
    for (v, s, until) in words {
        if !living(world, v) {
            world.consequence.owed_word.retain(|&(x, y, _)| !(x == v && y == s));
            continue;
        }
        let (he, _, _) = crate::systems::consequence::pro(world, v);
        let why = if !living(world, s) {
            Some(format!("{} is dead", world.name_of(s)))
        } else if world.has::<Sentence>(s) {
            Some(format!("{} is in the cells already", world.name_of(s)))
        } else if world.has::<Sentence>(v) {
            Some(format!("{he} was taken in first"))
        } else if crate::systems::presence::asleep(world, v) || crate::systems::wounded::is_down(world, v) {
            // He tells when he is up (or the word runs out below).
            (until <= now).then(|| {
                format!(
                    "{he} lay {} and the day went by",
                    if crate::systems::presence::asleep(world, v) { "asleep" } else { "hurt" }
                )
            })
        } else if !tells_the_law(world, v, s) {
            keeps_from_law(world, v, s)
        } else if world.comp::<Brain>(v).is_some_and(|b| b.lod == crate::components::Lod::Statistical)
            && stat_free(world, v)
            && law_desk(world, v).is_some()
        {
            // Off screen, no brain to carry the goal: his first free waking
            // hour with a desk open in reach is the walk to it.
            let desk = law_desk(world, v).map(|(d, _)| d);
            told_at_desk(world, v, s, desk, true);
            None
        } else if until <= now {
            match law_desk(world, v) {
                // Pressure pass round 9, Y2 (`OPEN-250`): no desk within a
                // walk, the word waits on the beat (`tell_the_beat`, the
                // street's way to the law) for as long as the sighting is
                // fresh to it (`[crime.case] tell_days`); dropped then. V17
                // `night_shift_24245 d22`: "never told: the law is out of
                // reach" at 07:00, then a guard on the street told at 23:00.
                None => match beat_until(world, v, s).filter(|&t| t > now) {
                    Some(t) => {
                        if let Some(x) = world.consequence.owed_word.iter_mut().find(|x| x.0 == v && x.1 == s) {
                            x.2 = t;
                        }
                        None
                    }
                    None => Some(
                        "the law is out of reach (no desk of it within a walk, and no guard on the beat came by)"
                            .to_string(),
                    ),
                },
                Some((d, _)) => Some(format!("the day went by and {he} never got to {}", world.name_of(d))),
            }
        } else {
            None
        };
        if let Some(why) = why {
            drop_word(world, v, s, &why);
        }
    }
}

/// M15 W24: is this an Attack's crime by a hunter on its Hunt's target,
/// with `[hunt] street_silence` on?
pub fn street_silence_on(world: &World, actor: EntityId, object: Option<EntityId>, crime: Crime) -> bool {
    world.config.hunt.street_silence
        && matches!(crime, Crime::Murder | Crime::Assault)
        && object.is_some()
        && world.hunts.get(&actor).is_some_and(|s| Some(s.target) == object)
}

/// M15 W24: a witness the street keeps quiet: one holding a grudge on the
/// victim, or a Friend of the actor.
pub fn silent_witness(world: &World, w: EntityId, actor: EntityId, victim: Option<EntityId>) -> bool {
    victim.is_some_and(|v| crate::systems::grudges::holds(world, w, v, 0.0))
        || world.edge(w, actor).is_some_and(|e| e.kind == crate::components::RelKind::Friend)
}

/// File (or refresh) the one open report per `(suspect, crime)`.
pub fn file_report(world: &mut World, crime: Crime, suspect: EntityId, witness: Option<EntityId>) {
    file_report_as(world, crime, suspect, witness, None);
}

/// `file_report` with the Report line's text and extra actors given (Round 6
/// V3: the law's case on a found body, a guard taking a victim's word);
/// `None` is the plain "X reported Y for Z". True when a new report filed.
pub fn file_report_as(
    world: &mut World,
    crime: Crime,
    suspect: EntityId,
    witness: Option<EntityId>,
    line: Option<(String, &[EntityId])>,
) -> bool {
    // Pressure pass round 2, R1: a fare is settled at the gate
    // (`systems::fares`), nobody's warrant.
    if crime == Crime::FareEvasion && crate::systems::fares::on(world) {
        return false;
    }
    let tick = world.tick;
    // Round 7 W2 (`OPEN-238`): a robbed sleeper's word reached the law.
    if let Some(w) = witness {
        crate::systems::consequence::word_given(world, crime, suspect, w);
    }
    if world.refresh_open_report(crime, suspect, witness) {
        return false;
    }
    world.push_report(CrimeReport { crime, suspect, witness, tick, resolved: false });
    // M13 D38: a new Dealing report (a refreshed one is the same warrant).
    if crime == Crime::Dealing {
        world.stats.current.dealing_reports += 1;
    }
    match line {
        Some((text, actors)) => {
            let mut all: Vec<EntityId> = vec![suspect];
            all.extend(actors.iter().copied().filter(|&a| a != suspect));
            world.push_event(EventKind::Report, &all, text);
        }
        None => {
            let who = witness.map_or("the city".to_string(), |w| world.name_of(w));
            world.push_event(
                EventKind::Report,
                &[suspect],
                format!("{who} reported {} for {}", world.name_of(suspect), crime.label()),
            );
        }
    }
    // Fix pass (phase 2 review): sleeping rough is not gang business; a
    // Vagrancy report against a member must not feed the wanted-gang log,
    // the pressure or the district's top gang.
    if crime != Crime::Vagrancy && crime != Crime::FareEvasion {
        crate::systems::law_brain::log_report(world, suspect);
    }
    true
}

/// Is `a` at work: on shift inside its employer, or on a Work plan (the
/// commute to or from a shift included)? A person at work is not loitering.
pub fn at_work(world: &World, a: EntityId) -> bool {
    let Some(job) = world.comp::<Job>(a) else { return false };
    if job.role == Role::Guard {
        return false;
    }
    let inside = job.employer.is_some() && job.employer == world.comp::<Position>(a).and_then(|p| p.building);
    (inside && job.on_shift(world.tick_of_day()))
        || world
            .comp::<Brain>(a)
            .is_some_and(|b| b.plan.as_ref().is_some_and(|p| p.goal == crate::components::GoalKind::Work))
}

/// Is Vagrancy the only thing `s` is wanted for, while `s` is at work? Such a
/// suspect is not chased: a night-shift worker with no bed is not loitering.
fn vagrant_at_work(world: &World, s: EntityId) -> bool {
    if !at_work(world, s) {
        return false;
    }
    // Scale S1: from the report index (this walked every report).
    world.wanted_only_for_vagrancy(s)
}

/// A wanted suspect the law can chase now: free, uncuffed and seen by a
/// guard within `suspect_seen_ticks`. Callers pair it with an open report.
pub(super) fn located(world: &World, s: EntityId) -> bool {
    let window = world.config.crime.suspect_seen_ticks;
    let tick = world.tick;
    // L1: a corpse keeps its Position and Identity (`is_alive`), and guards
    // walked hours to arrest the dead. The cheap checks first: the vagrancy
    // test walks the reports (a pure conjunction, reordered).
    (living(world, s))
        && !world.has::<Sentence>(s)
        && world.comp::<Brain>(s).is_some_and(|b| b.cuffed_by.is_none())
        && world.last_seen().get(&s).is_some_and(|&(_, t)| tick.saturating_sub(t) <= window)
        && !vagrant_at_work(world, s)
}
