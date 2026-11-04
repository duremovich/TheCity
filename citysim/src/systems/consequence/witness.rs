use super::*;

// ---------------------------------------------------------------------------
// Witnesses
// ---------------------------------------------------------------------------

/// `law::raise_crime_mult`: civilian `w` noted `actor`'s `crime` (the
/// Witness event written). One reaction; the culprit may see him.
pub fn witnessed(world: &mut World, w: EntityId, actor: EntityId, crime: Crime, object: Option<EntityId>) {
    let Some(c) = cfg(world).cloned() else { return };
    if crate::systems::law::is_petty(crime) || !person(world, actor) || !person(world, w) {
        return;
    }
    if !crate::systems::demography::is_adult(world, w) || object == Some(w) {
        return;
    }
    // His own crew's work is not his to sell or tell.
    if world.gang_of(w).is_some() && world.gang_of(w) == world.gang_of(actor) {
        return;
    }
    let t = traits(world, w);
    let violent = matches!(crime, Crime::Murder | Crime::Assault | Crime::Abduction | Crime::Manslaughter);
    // Round 8 X6 (`OPEN-217`): a held witness (a cellmate's crime: X1's
    // `sees_scene` keeps the held to their own cells) has no door to run
    // for and nobody outside to tell: he tells the jailer (the report, the
    // case is `law::cell_case`'s) or keeps clear of the man (V16 and X1's
    // 20k `jailed_10485 d24 06:18`: "saw ...'s Assault and ran").
    if world.has::<Sentence>(w) {
        held_witness(world, &c, w, actor, crime, violent, t);
        return;
    }
    let deed = crate::systems::gossip::deed_of_crime(crime);
    let bm_ok = t.lawful <= c.bm_lawful_max
        && world.purse(Some(w)) <= c.bm_broke_coins
        && world.consequence.blackmail.len() < c.bm_max_open
        && !world.consequence.blackmail.iter().any(|b| b.witness == w || (b.culprit == w && b.witness == actor));
    let weights = [
        c.w_report * t.lawful,
        if violent { c.w_flee * (1.0 - t.courage) } else { 0.0 },
        if deed.is_some() { c.w_gossip * t.social } else { 0.0 },
        if bm_ok { c.w_blackmail * (1.0 - t.lawful) * (0.5 + t.greed) } else { 0.0 },
    ];
    let u = hash(world, P_REACT, w, world.tick ^ (key(actor) << 7));
    let (wn, an) = (world.name_of(w), world.name_of(actor));
    let label = crime.label();
    match pick(&weights, u) {
        Some(1) => {
            crate::systems::social::make_enemy(world, w, actor, c.flee_affinity);
            if let Some(n) = world.comp_mut::<Needs>(w) {
                n.safety = 0.0;
            }
            let text = format!("{wn} saw {an}'s {label} and ran");
            world.push_event(EventKind::WitnessReacted, &[w, actor], text);
            ownership::jobs2_row(world).wr_flee += 1;
        }
        Some(2) => {
            if let Some(d) = deed {
                let told = tell(world, &c, w, actor, d, object, crime);
                if told > 0 {
                    let his = pro(world, w).2;
                    let text = format!("{wn} told {told} of {his} people what {an} did (the {label})");
                    world.push_event(EventKind::WitnessReacted, &[w, actor], text);
                    ownership::jobs2_row(world).wr_gossip += 1;
                }
            }
        }
        Some(3) => {
            let next_day = world.day() + u64::from(c.bm_wait_days);
            world.consequence.blackmail.push(Blackmail {
                witness: w,
                culprit: actor,
                crime,
                next_day,
                demands: 0,
                tries: 0,
                paid: 0,
            });
            let him = pro(world, w).1;
            let text = format!("{wn} saw {an}'s {label} and kept it to {him}self: it is worth something");
            world.push_event(EventKind::WitnessReacted, &[w, actor], text);
            ownership::jobs2_row(world).wr_blackmail += 1;
        }
        _ => ownership::jobs2_row(world).wr_report += 1,
    }
    // The culprit sees who saw him (the pair's edge sours; the Silence pass reads it).
    // The pressure pass R4 (OPEN-219): no line of its own (781 a month at
    // 20k, one a witness a blow); the soured edge and the counter carry it.
    if hash(world, P_LEARN, actor, world.tick ^ key(w)) < c.learn_p {
        crate::systems::social::adjust(world, actor, w, -0.1, 0.0);
        ownership::jobs2_row(world).wr_learned += 1;
    }
}

/// X6: the held witness's reaction (`witnessed`): the report (no line; the
/// case is the cells'), or, a violent crime, he keeps clear of the actor
/// (an Enemy, his safety down to half): one weighted draw as outside.
fn held_witness(
    world: &mut World,
    c: &ConsequenceCfg,
    w: EntityId,
    actor: EntityId,
    crime: Crime,
    violent: bool,
    t: Traits,
) {
    let weights = [c.w_report * t.lawful, if violent { c.w_flee * (1.0 - t.courage) } else { 0.0 }];
    let u = hash(world, P_REACT, w, world.tick ^ (key(actor) << 7));
    if pick(&weights, u) == Some(1) {
        crate::systems::social::make_enemy(world, w, actor, c.flee_affinity);
        if let Some(n) = world.comp_mut::<Needs>(w) {
            n.safety = n.safety.min(0.5);
        }
        let place = world.comp::<Position>(w).and_then(|p| p.building).map_or_else(String::new, |b| world.name_of(b));
        let text = format!(
            "{} saw {}'s {} in the cells of {place} and kept to the far wall, away from {}",
            world.name_of(w),
            world.name_of(actor),
            crime.label(),
            pro(world, actor).1
        );
        world.push_event(EventKind::WitnessReacted, &[w, actor], text);
        ownership::jobs2_row(world).wr_held_clear += 1;
    } else {
        ownership::jobs2_row(world).wr_report += 1;
    }
}

/// The gossip: `w`'s closest people (Friend, Family, Spouse, Parent, best
/// affinity first) hear the deed at once, hops 1. Returns how many heard.
fn tell(
    world: &mut World,
    c: &ConsequenceCfg,
    w: EntityId,
    actor: EntityId,
    deed: crate::word::Deed,
    object: Option<EntityId>,
    crime: Crime,
) -> u32 {
    let mut near: Vec<(f32, EntityId)> = world
        .neighbours(w)
        .filter(|&o| o != actor && Some(o) != object && world.has::<Brain>(o))
        .filter_map(|o| {
            world
                .edge(w, o)
                .filter(|e| matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Spouse | RelKind::Parent))
                .map(|e| (e.affinity, o))
        })
        .collect();
    near.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let hop = world.config.gossip.hop_salience;
    let salience = crate::systems::law::crime_salience(crime);
    let now = world.tick;
    let mut told = 0;
    for (_, o) in near.into_iter().take(usize::from(c.gossip_tell)) {
        let trust = world.edge(o, w).map_or(0.0, |e| e.trust);
        let entry = MemoryEntry {
            subject: Some(actor),
            salience: salience * hop,
            valence: -salience * hop,
            second_hand: true,
            deed: Some(deed),
            object,
            hops: 1,
            conf: 0.5 + 0.5 * trust,
            ..MemoryEntry::blank(MemoryKind::Rumour, now)
        };
        let out = crate::systems::memory::hear_entry(world, o, entry);
        if matches!(
            out,
            crate::systems::memory::HeardInsert::Inserted | crate::systems::memory::HeardInsert::Contradicted
        ) {
            told += 1;
            world.stats.current.word.rumours_heard += 1;
            if world.edge(o, actor).is_some() {
                let sev = world.config.gossip.deed_sev.get(deed);
                crate::systems::social::adjust(world, o, actor, -0.1 * sev, 0.0);
            }
        }
    }
    told
}

/// Round 7 W2 (`OPEN-238`): `sleeper` paid `coins` to `extorter`'s
/// shakedown asleep (`gang::extort`).
pub fn slept_through(world: &mut World, sleeper: EntityId, extorter: EntityId, coins: i64) {
    crate::systems::ownership::jobs2_row(world).sleeper_paid += 1;
    let now = world.tick;
    world.consequence.slept_through.push((sleeper, extorter, now, coins));
}

/// Round 7 W5 (`OPEN-237`): `w` holds a `WasRobbed` of `actor` from the
/// last `[crime.case] tell_days`.
pub fn robbed_by_within(world: &World, w: EntityId, actor: EntityId) -> bool {
    let since = world.tick.saturating_sub(world.config.crime.case.tell_days * crate::time::TICKS_PER_DAY);
    world.comp::<crate::components::Memory>(w).is_some_and(|m| {
        m.entries.iter().any(|e| e.kind == MemoryKind::WasRobbed && e.subject == Some(actor) && e.tick >= since)
    })
}

/// Round 8 X1 (`OPEN-246`): `w` means to tell the law of `actor`: the
/// promise stands `[crime.case] word_hours` (one per pair, a second robbery
/// renews it); `law::keep_word` keeps or drops it with his line.
fn promise(world: &mut World, w: EntityId, actor: EntityId) {
    let until = world.tick + world.config.crime.case.word_hours * crate::time::TICKS_PER_HOUR;
    if let Some(x) = world.consequence.owed_word.iter_mut().find(|x| x.0 == w && x.1 == actor) {
        x.2 = x.2.max(until);
        return;
    }
    world.consequence.owed_word.push((w, actor, until));
    ownership::jobs2_row(world).word_meant += 1;
}

/// Round 7 W5 (`OPEN-237`; `gang::extort`): a payer awake at the door
/// paid `coins` to `actor`'s shakedown of `home`: his line, as the woken
/// sleeper's (`woke`): he means to tell the law (the word owed, read by
/// `word_given`) or he tells nobody and why (`law::keeps_from_law`). V15
/// `ceo_20314 d23 08:26`: shaken down at home twice by one man in four days
/// with 570c in the purse, and nothing in the diary but the money line.
pub fn paid_awake(world: &mut World, w: EntityId, actor: EntityId, home: EntityId, coins: i64, again: bool) {
    if !living(world, w) || !world.has::<Brain>(w) {
        return;
    }
    let now = world.tick;
    let (wn, an, at) = (world.name_of(w), world.name_of(actor), world.name_of(home));
    let again = if again { ", again" } else { "" };
    let _ = now;
    // Round 8 X1: a robot at the door detained him: nothing left to tell.
    let text = if world.has::<Sentence>(actor) {
        format!("{wn} paid {an}'s shakedown at {at}{again} ({coins}c); the law has {an} already")
    } else if crate::systems::law::tells_the_law(world, w, actor) {
        promise(world, w, actor);
        let he = pro(world, w).0;
        format!("{wn} paid {an}'s shakedown at {at}{again} ({coins}c); {he} means to tell the law")
    } else {
        let why = crate::systems::law::keeps_from_law(world, w, actor).unwrap_or_default();
        format!("{wn} paid {an}'s shakedown at {at}{again} ({coins}c); tells nobody: {why}")
    };
    world.push_event(EventKind::WitnessReacted, &[w, actor], text);
}

/// Round 7 W2 (`OPEN-238`), every `[crime.case] find_every_ticks` (with
/// the find pass): a sleeper robbed in his sleep and awake now finds his
/// purse short. He learns who from the block (the shakedown was at his
/// door): `WasRobbed`, the soured edge and a victim's first-hand sighting
/// for each (what the hue, the tell and ReportCrime read), and a line: he
/// means to tell the law (an on-duty guard in earshot takes it at once),
/// or why he tells nobody (`law::keeps_from_law`). V15 `courier_50890
/// d21-23`: robbed four times asleep in three days, and told nobody.
pub fn woke(world: &mut World) {
    if world.consequence.slept_through.is_empty() {
        return;
    }
    let now = world.tick;
    let mut ready: Vec<(EntityId, EntityId, Tick, i64)> = Vec::new();
    let mut keep: Vec<(EntityId, EntityId, Tick, i64)> = Vec::new();
    for x in std::mem::take(&mut world.consequence.slept_through) {
        let gone = !living(world, x.0) || world.has::<Sentence>(x.0);
        let up = !crate::systems::presence::asleep(world, x.0);
        // A day asleep (the stand-in's night, a coma) is forgotten.
        let stale = now.saturating_sub(x.2) > crate::time::TICKS_PER_DAY;
        if gone || stale {
            continue;
        }
        if up {
            ready.push(x);
        } else {
            keep.push(x);
        }
    }
    world.consequence.slept_through = keep;
    let mut sleepers: Vec<EntityId> = ready.iter().map(|x| x.0).collect();
    sleepers.sort_unstable();
    sleepers.dedup();
    for k in sleepers {
        let mine: Vec<(EntityId, EntityId, Tick, i64)> = ready.iter().copied().filter(|x| x.0 == k).collect();
        let coins: i64 = mine.iter().map(|x| x.3).sum();
        let mut by: Vec<EntityId> = Vec::new();
        for x in &mine {
            if !by.contains(&x.1) {
                by.push(x.1);
            }
        }
        for &a in &by {
            world.remember(k, MemoryKind::WasRobbed, Some(a), 0.7, -0.7, false);
            crate::systems::social::robbed_by(world, k, a);
            if living(world, a) {
                world.remember_crime(k, a, Crime::Extortion, 0.6, Some(k));
            }
        }
        crate::systems::ownership::jobs2_row(world).sleeper_found += 1;
        let (_, place) = place_of(world, k);
        let who = match by.len() {
            1 => format!("{} shook the block down in the night", world.name_of(by[0])),
            _ => format!(
                "{} and {} shook the block down in the night",
                by[..by.len() - 1].iter().map(|&a| world.name_of(a)).collect::<Vec<_>>().join(", "),
                world.name_of(by[by.len() - 1])
            ),
        };
        let kn = world.name_of(k);
        // The one he tells of: the first of them he would name.
        let teller =
            by.iter().copied().find(|&a| !world.has::<Sentence>(a) && crate::systems::law::tells_the_law(world, k, a));
        let held = by.iter().copied().find(|&a| world.has::<Sentence>(a));
        let text = match (teller, held) {
            (Some(a), _) => {
                promise(world, k, a);
                let he = pro(world, k).0;
                format!("{kn} woke {place} to find {coins}c gone from the purse: {who}; {he} means to tell the law")
            }
            (None, Some(a)) => {
                let an = world.name_of(a);
                format!("{kn} woke {place} to find {coins}c gone from the purse: {who}; the law has {an} already")
            }
            (None, None) => {
                crate::systems::ownership::jobs2_row(world).sleeper_quiet += 1;
                let why = crate::systems::law::keeps_from_law(world, k, by[0]).unwrap_or_default();
                format!("{kn} woke {place} to find {coins}c gone from the purse: {who}; tells nobody: {why}")
            }
        };
        let mut actors = vec![k];
        actors.extend(by.iter().copied());
        world.push_event(EventKind::WitnessReacted, &actors, text);
        // An on-duty guard in earshot on the street takes it now.
        if let (Some(a), Some(tile)) = (teller, world.comp::<Position>(k).map(|p| p.tile)) {
            crate::systems::law::hue_and_cry(world, k, a, Crime::Extortion, tile);
        }
    }
}

/// Round 7 W2: a report filed (`law::file_report_as`): a robbed sleeper's
/// word reached the law (`sleeper_reports`).
pub fn word_given(world: &mut World, crime: Crime, suspect: EntityId, witness: EntityId) {
    // Round 8 X1: any crime of the man he promised to name keeps the word
    // (the tell names the freshest sighting, not always the shakedown).
    let _ = crime;
    if world.consequence.owed_word.is_empty() {
        return;
    }
    let before = world.consequence.owed_word.len();
    world.consequence.owed_word.retain(|&(v, s, _)| !(v == witness && s == suspect));
    let n = before - world.consequence.owed_word.len();
    if n > 0 {
        let row = crate::systems::ownership::jobs2_row(world);
        row.sleeper_reports += n as u32;
        row.word_kept += n as u32;
    }
}

/// ReportCrime's reader: `w` is keeping `culprit`'s crime to sell.
pub fn keeps_quiet(world: &World, w: EntityId, culprit: EntityId) -> bool {
    world.consequence.blackmail.iter().any(|b| b.witness == w && b.culprit == culprit)
}

// ---------------------------------------------------------------------------
// Blackmail
// ---------------------------------------------------------------------------

pub(super) fn blackmail_daily(world: &mut World, c: &ConsequenceCfg) {
    let today = world.day();
    let mut i = 0;
    while i < world.consequence.blackmail.len() {
        let b = world.consequence.blackmail[i].clone();
        let done = b.demands >= c.bm_max_demands;
        if done || b.tries >= c.bm_tries || !living(world, b.witness) || !living(world, b.culprit) {
            world.consequence.blackmail.remove(i);
            if done && living(world, b.witness) && living(world, b.culprit) {
                let text = format!(
                    "{} keeps {}'s secret now: {}c paid for the {} in all",
                    world.name_of(b.witness),
                    world.name_of(b.culprit),
                    b.paid,
                    b.crime.label()
                );
                world.push_event(EventKind::Blackmail, &[b.witness, b.culprit], text);
            }
            continue;
        }
        let ready = b.next_day <= today
            && free(world, b.witness)
            && free(world, b.culprit)
            && !world.visits.contains_key(&b.witness);
        if ready {
            world.consequence.blackmail[i].tries += 1;
            if !schedule(world, b.witness, b.culprit, VisitWhy::Blackmail, c.bm_hour_from, c.bm_hour_to)
                && !crate::systems::household::on(world)
            {
                blackmailed(world, b.witness, b.culprit, false);
            }
        }
        i += 1;
    }
}
