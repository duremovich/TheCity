use super::*;

/// The roll stream of one fight: a world-stream salt, hashed per draw.
#[derive(Clone, Debug)]
pub(super) struct Dice {
    salt: u64,
    n: u64,
}

impl Dice {
    fn roll(&mut self, world: &World) -> f32 {
        self.n += 1;
        hash(world, PURPOSE_BEAT, self.salt, self.n)
    }
}

pub(super) fn pick_weighted(r: f32, w: &[f32]) -> usize {
    let total: f32 = w.iter().sum();
    if total <= 0.0 {
        return 0;
    }
    let mut x = r * total;
    for (i, &v) in w.iter().enumerate() {
        if x < v {
            return i;
        }
        x -= v;
    }
    w.len() - 1
}

/// `a` (the starter) against `b`, run to the end in one call (docs/COMBAT.md
/// § 1, off camera, and the pit's bout). Writes the wounds, the memories,
/// the skills, the deaths and the downed; returns the outcome. C3a: the
/// same duel as a live engagement's ([`duel_start`], [`duel_beats`],
/// [`duel_end`]), its beats all at once.
pub fn resolve_now(world: &mut World, a: EntityId, b: EntityId, mods: crate::systems::law::FightMods) -> FightOutcome {
    let mut d = duel_start(world, a, b, &mods);
    let mut left = u32::MAX;
    duel_beats(world, &mut d, &mods, &mut left, None);
    duel_end(world, d, &mods)
}

/// One duel in progress: the two fighters, the dice, the beats run.
#[derive(Clone, Debug)]
pub(super) struct Duel {
    pub(super) f: [Fighter; 2],
    dice: Dice,
    order: [usize; 2],
    pub(super) beat: u8,
    notes: [Option<&'static str>; 2],
    /// The tick it began (a live duel's span).
    since: Tick,
    /// Where the two stood at the start (a live duel: one moved has left)
    /// and whether they were within `reach_tiles` of each other then.
    pub(super) at: [(crate::components::TilePos, Option<EntityId>); 2],
    pub(super) near: bool,
    pub(super) names: [String; 2],
    /// C3b: the rest of each side, striking the other side's front each
    /// beat (numbers count).
    pub(super) aid: [Vec<Fighter>; 2],
}

pub(super) fn duel_start(world: &mut World, a: EntityId, b: EntityId, mods: &crate::systems::law::FightMods) -> Duel {
    use rand::Rng;
    for x in [a, b] {
        seed_skills(world, x);
    }
    let (sa, sb) = (stance_for(world, a, true, mods), stance_for(world, b, false, mods));
    let notes = [carried_note(world, a), carried_note(world, b)];
    let bout = mods.why == Purpose::Bout;
    let f = [fighter(world, a, sa, true, mods.a_bonus, bout), fighter(world, b, sb, false, 0.0, bout)];
    // A bout is keyed on the pair and the tick (the pit drew on its own
    // `WordNs::Bout` stream before C2: the world stream stays the city's).
    let salt = if bout { key(a) ^ key(b).rotate_left(23) ^ world.tick } else { world.rng.world().random::<u64>() };
    let order = if f[1].reflex > f[0].reflex { [1, 0] } else { [0, 1] };
    let pos = |x: EntityId| world.comp::<Position>(x).map_or((Default::default(), None), |p| (p.tile, p.building));
    let near = crate::systems::law::near(world, a, b, world.config.combat.reach_tiles);
    Duel {
        f,
        dice: Dice { salt, n: 0 },
        order,
        beat: 0,
        notes,
        at: [pos(a), pos(b)],
        near,
        names: [world.name_of(a), world.name_of(b)],
        since: world.tick,
        aid: [Vec::new(), Vec::new()],
    }
}

/// C3b: the rest of each side behind the duel's fronts strike with them.
pub(super) fn duel_aid(
    world: &mut World,
    d: &mut Duel,
    sides: &[Vec<EntityId>; 2],
    mods: &crate::systems::law::FightMods,
) {
    for (k, side) in sides.iter().enumerate() {
        let front = d.f[k].id;
        let ids: Vec<EntityId> = side.iter().copied().filter(|&x| x != front).collect();
        d.aid[k] = aid_of(world, k, &ids, mods);
    }
}

/// The fighters of side `k` striking behind its front.
pub(super) fn aid_of(
    world: &mut World,
    k: usize,
    ids: &[EntityId],
    mods: &crate::systems::law::FightMods,
) -> Vec<Fighter> {
    let bout = mods.why == Purpose::Bout;
    let mut out = Vec::with_capacity(ids.len());
    for &x in ids {
        seed_skills(world, x);
        let st = stance_for(world, x, k == 0, mods);
        let bonus = if k == 0 { mods.a_bonus } else { 0.0 };
        let mut f = fighter(world, x, st, k == 0, bonus, bout);
        f.attacked = true;
        out.push(f);
    }
    out
}

/// Is a fighter standing (no end yet)?
pub(super) fn standing(f: &Fighter) -> bool {
    f.end.is_none()
}

/// The Sandevistan's extra strikes this beat: fired at the body's first
/// action in a fight (off cooldown: `sandy_beats` beats, the sanity cost).
fn sandy_extra(world: &mut World, f: &mut Fighter) -> u8 {
    if f.sandy == 0 {
        return 0;
    }
    if f.sandy_armed && f.sandy_left == 0 {
        f.sandy_armed = false;
        let c = &world.config.combat;
        f.sandy_left = c.sandy_beats;
        f.sandy_fired = true;
        let (cd, cost) = (Tick::from(c.sandy_cooldown_hours) * TICKS_PER_HOUR, c.sandy_sanity);
        let until = world.tick + cd;
        world.combat.sandy_cd.insert(f.id, until);
        if let Some(b) = world.comp_mut::<crate::components::Body>(f.id) {
            b.sanity = (b.sanity - cost).clamp(0.0, 1.0);
        }
        crate::systems::ownership::jobs2_row(world).sandy_uses += 1;
    }
    if f.sandy_left == 0 {
        return 0;
    }
    f.sandy_left -= 1;
    f.sandy
}

/// Run beats of `d`, each taken off `left`, until it ends (true) or `left`
/// runs out (false). `log`: a live engagement's ring log.
pub(super) fn duel_beats(
    world: &mut World,
    d: &mut Duel,
    mods: &crate::systems::law::FightMods,
    left: &mut u32,
    mut log: Option<&mut std::collections::VecDeque<BeatLog>>,
) -> bool {
    let lethal = mods.kill_p.is_some();
    let bout = mods.why == Purpose::Bout;
    let max_beats = beat_cap(&world.config.combat, d.f.iter().any(|x| x.human));
    let cap = world.config.combat.log_beats;
    let note = |log: &mut Option<&mut std::collections::VecDeque<BeatLog>>, tick, beat, by, on, blow| {
        if let Some(l) = log.as_deref_mut() {
            if l.len() >= cap.max(1) {
                l.pop_front();
            }
            l.push_back(BeatLog { tick, beat, by, on, blow });
        }
    };
    let blow_of = |f: &Fighter, took: usize| {
        if f.took.len() > took {
            f.took.last().map_or(Blow::Miss, |&(r, v)| Blow::Hit(r, v))
        } else {
            Blow::Miss
        }
    };
    loop {
        if d.f[0].end.is_some() || d.f[1].end.is_some() {
            return true;
        }
        if d.beat >= max_beats {
            // Out of beats: the lower morale gives way (flees if it can).
            let i = if d.f[0].morale < d.f[1].morale { 0 } else { 1 };
            let e = break_off(world, &d.f, i, lethal, &mut d.dice).unwrap_or(End::Yielded);
            d.f[i].end = Some(e);
            note(&mut log, world.tick, d.beat, d.f[i].id, d.f[1 - i].id, Blow::Broke(e));
            return true;
        }
        if *left == 0 {
            return false;
        }
        *left -= 1;
        let beat = d.beat;
        d.beat += 1;
        let order = d.order;
        // C3b: a fresh beat; the head count on each side (the fronts and
        // the standing behind them).
        for x in d.f.iter_mut().chain(d.aid.iter_mut().flatten()) {
            x.faced = 0;
        }
        let count = |v: &Vec<Fighter>| 1.0 + v.iter().filter(|x| standing(x)).count() as f32;
        let heads = [count(&d.aid[0]), count(&d.aid[1])];
        for i in order {
            let j = 1 - i;
            if d.f[0].end.is_some() || d.f[1].end.is_some() {
                break;
            }
            // C4: the human's order to break off, tried at the body's turn
            // (a flight on the break's roll; a yield the other side takes).
            if let Some(w) = d.f[i].want {
                let id = d.f[i].id;
                match w {
                    Want::Flee => {
                        if flee_roll(world, &d.f[i], &mut d.dice) {
                            d.f[i].end = Some(End::Fled);
                            clear_want(world, id);
                            note(&mut log, world.tick, d.beat, id, d.f[j].id, Blow::Broke(End::Fled));
                            break;
                        }
                        note(&mut log, world.tick, d.beat, id, d.f[j].id, Blow::Stuck);
                        // M18 P5: safe idle, cornered, gives up instead
                        if crate::systems::player::safe_idle(world, id) {
                            d.f[i].want = Some(Want::Yield);
                        }
                        continue;
                    }
                    Want::Yield => {
                        d.f[i].want = None;
                        clear_want(world, id);
                        if d.f[j].stance.takes_yield(lethal && i == 1) {
                            d.f[i].end = Some(End::Yielded);
                            note(&mut log, world.tick, d.beat, id, d.f[j].id, Blow::Broke(End::Yielded));
                            break;
                        }
                        note(&mut log, world.tick, d.beat, id, d.f[j].id, Blow::Refused);
                    }
                }
            }
            let c = &world.config.combat;
            // The break: flee, else yield, else cornered (fights on). C3b:
            // morale reads the numbers. C5: only once a blow has landed on
            // it or the numbers are against it (no free yield, `OPEN-296`);
            // the human's body never breaks on its nerve, it shakes.
            let morale = d.f[i].morale + c.numbers_morale * (heads[i] / heads[j]).ln();
            let line = c.break_at + c.break_shift[d.f[i].stance.i()];
            let pressed = (d.f[i].struck && d.f[i].swung) || heads[j] > heads[i];
            if d.f[i].human {
                d.f[i].shake = (line - morale).max(0.0);
            } else if !d.f[i].cornered && pressed && morale < line {
                if let Some(e) = break_off(world, &d.f, i, lethal, &mut d.dice) {
                    d.f[i].end = Some(e);
                    note(&mut log, world.tick, d.beat, d.f[i].id, d.f[j].id, Blow::Broke(e));
                    break;
                }
                d.f[i].cornered = true;
            }
            if d.f[i].stance == Stance::HoldBack && !d.f[i].attacked {
                continue;
            }
            if beat < d.f[i].ready_at {
                continue;
            }
            let n = 1 + sandy_extra(world, &mut d.f[i]);
            for _ in 0..n {
                if d.f[j].end.is_some() || d.f[i].end.is_some() {
                    break;
                }
                let took = d.f[j].took.len();
                let [f0, f1] = &mut d.f;
                let (att, tgt) = if i == 0 { (f0, f1) } else { (f1, f0) };
                strike(world, att, tgt, mods, &mut d.dice);
                referee(world, tgt, bout, took);
                note(&mut log, world.tick, d.beat, d.f[i].id, d.f[j].id, blow_of(&d.f[j], took));
                if let Some(e) = d.f[j].end {
                    note(&mut log, world.tick, d.beat, d.f[j].id, d.f[i].id, Blow::Broke(e));
                }
            }
            d.f[i].swung = true;
            tire(&mut d.f[i], world.config.combat.tire_beat);
        }
        // C3b, numbers count: the rest of each side strike the other front
        // (as many melee bodies as reach him, `crowd_reach` with the front;
        // every gun).
        let reach = world.config.combat.crowd_reach.max(1);
        for k in 0..2 {
            let mut melee = 1;
            for h in 0..d.aid[k].len() {
                if d.f[0].end.is_some() || d.f[1].end.is_some() {
                    break;
                }
                if !standing(&d.aid[k][h]) || beat < d.aid[k][h].ready_at {
                    continue;
                }
                if !ranged(world, d.aid[k][h].kind) {
                    if melee >= reach {
                        continue;
                    }
                    melee += 1;
                }
                let took = d.f[1 - k].took.len();
                let n = 1 + sandy_extra(world, &mut d.aid[k][h]);
                for _ in 0..n {
                    if d.f[1 - k].end.is_some() {
                        break;
                    }
                    strike(world, &mut d.aid[k][h], &mut d.f[1 - k], mods, &mut d.dice);
                }
                referee(world, &mut d.f[1 - k], bout, took);
                let by = d.aid[k][h].id;
                note(&mut log, world.tick, d.beat, by, d.f[1 - k].id, blow_of(&d.f[1 - k], took));
                if let Some(e) = d.f[1 - k].end {
                    note(&mut log, world.tick, d.beat, d.f[1 - k].id, by, Blow::Broke(e));
                }
                tire(&mut d.aid[k][h], world.config.combat.tire_beat);
            }
        }
    }
}

/// Fatigue: `amount` x the body's drain (none for a robot), to 1.
fn tire(f: &mut Fighter, amount: f32) {
    f.fatigue = (f.fatigue + amount * f.tire_mult).min(1.0);
}

/// C3b, C2's leftover: the pit's referee stops a bout at the first wound of
/// `bout_stop` or worse (the hurt one yields).
fn referee(world: &World, f: &mut Fighter, bout: bool, took: usize) {
    if bout
        && f.end.is_none()
        && f.took[took.min(f.took.len())..].iter().any(|t| t.1.index() >= world.config.combat.bout_stop)
    {
        f.end = Some(End::Yielded);
    }
}

/// The ended duel's outcome: the winner, a lethal purpose's finish, the
/// tail of the fight (`finish`).
pub(super) fn duel_end(world: &mut World, mut d: Duel, mods: &crate::systems::law::FightMods) -> FightOutcome {
    let (a, b) = (d.f[0].id, d.f[1].id);
    let lethal = mods.kill_p.is_some();
    let li = if d.f[0].end.is_some() { 0 } else { 1 };
    let (winner, loser) = (d.f[1 - li].id, d.f[li].id);
    d.f[1 - li].end = Some(End::Won);
    // A lethal purpose's win (a Hunt's strike) finishes the beaten body.
    if lethal && winner == a && d.f[li].end != Some(End::Dead) && !d.f[li].robot {
        let p = mods.kill_p.unwrap_or(0.0) * mods.kill_mult;
        if d.dice.roll(world) < p {
            d.f[li].end = Some(End::Dead);
        }
    }
    let ticks = world.tick.saturating_sub(d.since) as u32;
    // C3b: the fatigue each fighter carries out, the helpers' growth, the
    // crowd for the line, the lone body's odds.
    let now = world.tick;
    let gains = world.config.combat.clone();
    for f in d.f.iter().chain(d.aid[0].iter()).chain(d.aid[1].iter()) {
        if !f.robot && f.fatigue > 0.0 {
            world.combat.tired.insert(f.id, (f.fatigue, now));
        }
    }
    let mut crowd = [(0u8, 0u8); 2];
    for (k, side) in d.aid.iter().enumerate() {
        for h in side {
            crowd[k].0 = crowd[k].0.saturating_add(1);
            crowd[k].1 = crowd[k].1.saturating_add(h.landed);
            if let Some(s) = world.comp_mut::<Skills>(h.id) {
                s.melee = (s.melee + gains.melee_gain * f32::from(h.melee_hits)).min(1.0);
                s.shooting = (s.shooting.max(0.0) + gains.shooting_gain * f32::from(h.shots)).min(1.0);
            }
        }
    }
    let (n0, n1) = (d.aid[0].len(), d.aid[1].len());
    if mods.why != Purpose::Bout && (n0 == 0) != (n1 == 0) {
        let lone_won = (n0 == 0 && winner == a) || (n1 == 0 && winner == b);
        let row = crate::systems::ownership::jobs2_row(world);
        if n0.max(n1) == 1 {
            row.lone_1v2 += 1;
            row.lone_1v2_won += u32::from(lone_won);
        } else {
            row.lone_1v3 += 1;
            row.lone_1v3_won += u32::from(lone_won);
        }
    }
    let out = finish(world, d.f, winner, loser, (d.beat, ticks), mods, a, b, d.notes, crowd);
    world.last_fight = Some(out.clone());
    out
}

/// The broken side `i` gets away: flees on its legs, else yields when the
/// other side takes yields; `None`: cornered.
fn break_off(world: &World, f: &[Fighter; 2], i: usize, lethal: bool, dice: &mut Dice) -> Option<End> {
    if flee_roll(world, &f[i], dice) {
        return Some(End::Fled);
    }
    // Only the starter's purpose is lethal.
    let other = &f[1 - i];
    let lethal_here = lethal && i == 1;
    other.stance.takes_yield(lethal_here).then_some(End::Yielded)
}

/// A body set on getting away gets away: (`flee_p` + `flee_evade_w` x
/// evasion) x the legs' wounds; a Grave leg or two leg Wounds cannot.
fn flee_roll(world: &World, f: &Fighter, dice: &mut Dice) -> bool {
    let c = &world.config.combat;
    let fx = crate::systems::health::effects(world, f.id);
    let p = (c.flee_p + c.flee_evade_w * f.evasion) * fx.evade_mult;
    !lame(world, f.id) && dice.roll(world) < p
}

/// Too lame to run: a Grave leg wound, or two leg Wounds.
pub(super) fn lame(world: &World, id: EntityId) -> bool {
    world.comp::<Wounds>(id).is_some_and(|w| {
        w.list.iter().any(|x| x.region.is_leg() && x.severity == Severity::Grave)
            || w.list.iter().filter(|x| x.region.is_leg() && x.severity >= Severity::Wound).count() >= 2
    })
}

pub(super) fn strike(
    world: &mut World,
    att: &mut Fighter,
    tgt: &mut Fighter,
    mods: &crate::systems::law::FightMods,
    dice: &mut Dice,
) {
    tgt.attacked = true;
    // The under-18 rule, at every hit (CLAUDE.md).
    if crate::systems::ages::is_protected(world, tgt.id) {
        att.end = Some(End::Yielded);
        return;
    }
    let c = world.config.combat.clone();
    let mut row = kind_row(world, att.kind).cloned().unwrap_or_default();
    // C3b: gorilla arms make the bare hand a killer.
    if att.gorilla {
        row.damage[2] *= c.gorilla_grave;
        row.lethality = row.lethality.max(c.gorilla_lethality);
    }
    // C3b: fatigue on both, and the split defence of a body set upon by
    // several in one beat (no floor under the numbers but none).
    tgt.faced = tgt.faced.saturating_add(1);
    let split = (1.0 - c.split_evade * f32::from(tgt.faced - 1)).max(0.0);
    // C5: a shaking body (the human's, under its break line) hits and dodges worse.
    let attack = att.attack * (1.0 - c.tire_attack * att.fatigue) * shaken(&c, c.shake_attack, att.shake);
    let evade = (tgt.evasion + c.stance_evade[tgt.stance.i()])
        * (1.0 - c.tire_evade * tgt.fatigue)
        * split
        * shaken(&c, c.shake_evade, tgt.shake);
    let p = (c.hit_base + c.attack_w * attack + row.accuracy * att.quality + c.stance_attack[att.stance.i()]
        - c.evade_w * evade)
        .clamp(c.hit_min, c.hit_max);
    // C4 (§ 6): cover turns a ranged hit; melee ignores it.
    let p = if row.family == WeaponFamily::Ranged { p * (1.0 - tgt.cover) } else { p };
    if dice.roll(world) >= p {
        tgt.dodged += 1;
        return;
    }
    // C3b: a Kerenzikov's dodge charge turns the hit.
    if tgt.keren > 0 {
        tgt.keren -= 1;
        if dice.roll(world) < tgt.keren_p {
            tgt.dodged += 1;
            tgt.keren_used += 1;
            crate::systems::ownership::jobs2_row(world).keren_dodges += 1;
            return;
        }
    }
    att.landed += 1;
    tgt.struck = true;
    if row.family == WeaponFamily::Ranged {
        att.shots += 1;
    } else {
        att.melee_hits += 1;
    }
    att.morale += c.morale_landed;
    let region = Region::ALL[pick_weighted(dice.roll(world), &c.region_weights)];
    let mut w = row.damage;
    let q = att.quality.max(0.01);
    w[0] /= q;
    w[2] *= q;
    if row.family == WeaponFamily::Melee {
        w[2] *= 1.0 + c.strength_grave * att.strength;
    }
    // C2 (a): an old body breaks easier.
    w[2] *= 1.0 + c.age_grave * over_age(world, tgt.id);
    let mut sev = [Severity::Graze, Severity::Wound, Severity::Grave][pick_weighted(dice.roll(world), &w)];
    // C3b: a robot is wrecked, not wounded (a Wound counts 1, a Grave all).
    if tgt.robot {
        let n = match sev {
            Severity::Graze => 0,
            Severity::Wound => 1,
            Severity::Grave => c.robot_wreck_hits,
        };
        tgt.wreck = tgt.wreck.saturating_add(n);
        tgt.took.push((region, sev));
        if tgt.wreck >= c.robot_wreck_hits.max(1) {
            tgt.end = Some(End::Wrecked);
        }
        return;
    }
    // Armour on the region takes it down a severity (a graze stopped).
    let armour = if region == Region::Torso && tgt.vest { tgt.skin.max(c.vest_torso) } else { tgt.skin };
    if armour > 0.0 && dice.roll(world) < armour * c.armour_w {
        match sev.down() {
            Some(s) => sev = s,
            None => return,
        }
    }
    // C3b: a chrome limb takes the hit: a graze turned, a Wound wears it, a
    // Grave wrecks it; no flesh is wounded.
    let limb = if region.is_arm() {
        tgt.chrome_arms
    } else if region.is_leg() {
        tgt.chrome_legs
    } else {
        None
    };
    if let Some(a) = limb {
        tgt.morale -= c.morale_hit[sev.index()] * 0.5;
        let cond = world.comp::<crate::components::Asset>(a).map_or(0, |x| x.condition);
        let wrecked = match sev {
            Severity::Graze => false,
            Severity::Wound => cond <= c.chrome_wear,
            Severity::Grave => true,
        };
        if wrecked {
            let why = format!("smashed by {} in a fight", world.name_of(att.id));
            crate::systems::assets::wreck(world, a, &why);
            crate::systems::ownership::jobs2_row(world).chrome_wrecked += 1;
            if region.is_arm() {
                tgt.chrome_arms = None;
            } else {
                tgt.chrome_legs = None;
            }
            let base = fighter(world, tgt.id, tgt.stance, false, 0.0, mods.why == Purpose::Bout);
            tgt.attack = base.attack + tgt.bonus;
            tgt.evasion = base.evasion;
        } else if sev == Severity::Wound {
            if let Some(x) = world.comp_mut::<crate::components::Asset>(a) {
                x.condition = cond - c.chrome_wear;
                // Its value from its condition, as the daily wear sets it.
                x.value = x.list * i64::from(x.condition) / 100;
            }
            // The worn implant's value and effective tier (the Kit) read again.
            crate::systems::assets::rekit(world, tgt.id);
        }
        return;
    }
    let subdue = att.stance == Stance::Subdue;
    let mut ko = false;
    if subdue && sev == Severity::Grave {
        // A knockout replaces a Grave wound where it can.
        sev = Severity::Wound;
        ko = true;
    }
    // C5: a subdue stance goes for the knockout: any blow landed puts the
    // body down on `subdue_ko_p` x (1 + its fatigue).
    if subdue && !ko {
        ko = dice.roll(world) < c.subdue_ko_p * (1.0 + tgt.fatigue);
    }
    let mut dead = false;
    if sev == Severity::Grave && matches!(region, Region::Head | Region::Torso) && !subdue {
        let p = row.lethality * q * mods.kill_mult;
        dead = dice.roll(world) < p;
    }
    if !dead && !ko && region == Region::Head {
        let fx = crate::systems::health::effects(world, tgt.id);
        let m = if row.blunt || att.gorilla { c.blunt_ko_mult } else { 1.0 };
        ko = dice.roll(world) < c.ko_p[sev.index()] * m + fx.ko_add;
    }
    tgt.took.push((region, sev));
    tgt.morale -= c.morale_hit[sev.index()];
    tire(tgt, c.tire_wound[sev.index()]);
    // C3b: hurt, the Kerenzikov's second charge comes.
    if sev >= Severity::Wound && !tgt.hurt_once && tgt.keren_p > 0.0 {
        tgt.hurt_once = true;
        tgt.keren += 1;
    }
    if dead {
        tgt.end = Some(End::Dead);
        return;
    }
    // A bleed-out from it is a Murder when the starter's side dealt it to
    // the side set upon and the striker is no guard.
    // A robot's wound is its owner's doing, never a Murder.
    let murder = att.side0 && !tgt.side0 && !att.robot && !crate::systems::law::is_guard(world, att.id);
    let by =
        if att.robot { world.comp::<crate::components::Asset>(att.id).and_then(|x| x.owner) } else { Some(att.id) };
    let blood = add_wound(world, tgt.id, region, sev, by, murder, row.blunt);
    // C2: a Grave limb hit from a blade or a gun may take the limb (C3b:
    // the kind's `sever_p`).
    if sev == Severity::Grave && (region.is_arm() || region.is_leg()) && !row.blunt && dice.roll(world) < row.sever_p {
        let how = if row.family == WeaponFamily::Ranged { "shot to ruin" } else { "hacked off" };
        lose_limb(world, tgt.id, region, &format!("{how} by {} with a {}", world.name_of(att.id), row.id));
    }
    // The wound's effects reach the next beat.
    let base = fighter(world, tgt.id, tgt.stance, false, 0.0, mods.why == Purpose::Bout);
    tgt.attack = base.attack + tgt.bonus;
    tgt.evasion = base.evasion;
    if blood <= 0.0 {
        tgt.end = Some(End::Downed);
    } else if ko {
        tgt.end = Some(End::KnockedOut);
    }
}

// Put a wound on `id` (bleeding by the table: the torso from a Wound up,
/// any Grave; a blunt blow (fists, a club) bruises and breaks, never bleeds)
/// and the blood it takes at once. Returns the blood left.
pub fn add_wound(
    world: &mut World,
    id: EntityId,
    region: Region,
    severity: Severity,
    by: Option<EntityId>,
    murder: bool,
    blunt: bool,
) -> f32 {
    // No child is ever harmed (CLAUDE.md): the backstop.
    if crate::systems::ages::is_protected(world, id) {
        return world.comp::<Wounds>(id).map_or(1.0, |w| w.blood);
    }
    let c = &world.config.combat;
    let bleeding = !blunt && (severity == Severity::Grave || (region == Region::Torso && severity == Severity::Wound));
    let loss = if blunt { 0.0 } else { c.blood_hit[severity.index()] };
    let heal_at = world.tick + Tick::from(c.heal_days[severity.index()]) * TICKS_PER_DAY;
    let w = Wound { region, severity, bleeding, since: world.tick, treated: false, by, murder, heal_at, aided: false };
    if !world.has::<Wounds>(id) {
        world.insert(id, Wounds::default());
    }
    world.hurt.insert(id);
    let mut left = 1.0;
    if let Some(x) = world.comp_mut::<Wounds>(id) {
        x.list.push(w);
        x.blood = (x.blood - loss).max(0.0);
        left = x.blood;
    }
    if region.is_leg() {
        crate::systems::assets::rekit(world, id);
    }
    left
}

/// The end of a fight: the deaths and the downed, the memories, drift,
/// skills, the weapons' tail, the line.
#[allow(clippy::too_many_arguments)]
pub(super) fn finish(
    world: &mut World,
    f: [Fighter; 2],
    winner: EntityId,
    loser: EntityId,
    (beats, ticks): (u8, u32),
    mods: &crate::systems::law::FightMods,
    a: EntityId,
    b: EntityId,
    notes: [Option<&'static str>; 2],
    crowd: [(u8, u8); 2],
) -> FightOutcome {
    let li = if f[0].id == loser { 0 } else { 1 };
    let loser_end = f[li].end.unwrap_or(End::Yielded);
    let bout = mods.why == Purpose::Bout;
    let robot = f[0].robot || f[1].robot;
    if !bout && !robot {
        sequel(world, a, b, winner, loser, mods);
    } else if robot {
        robot_sequel(world, winner, loser);
    }
    // Growth: hits landed and dodges (C1, replacing the winner's flat +0.01).
    let c = world.config.combat.clone();
    for x in &f {
        if let Some(s) = world.comp_mut::<Skills>(x.id) {
            s.melee = (s.melee + c.melee_gain * f32::from(x.melee_hits)).min(1.0);
            s.shooting = (s.shooting.max(0.0) + c.shooting_gain * f32::from(x.shots)).min(1.0);
            s.evasion = (s.evasion.max(0.0) + c.evasion_gain * f32::from(x.dodged)).min(1.0);
        }
    }
    let mut died = loser_end == End::Dead;
    let mut end = loser_end;
    if loser_end == End::Downed {
        let murder = loser == b && !crate::systems::law::is_guard(world, a);
        if !go_down(world, loser, Some(winner), murder) {
            died = true;
            end = End::Dead;
        }
    }
    if died && f[li].robot {
        died = false;
        end = End::Wrecked;
    }
    if died {
        // A robot that kills passes its owner as the killer.
        let killer = if f[1 - li].robot {
            world.comp::<crate::components::Asset>(winner).and_then(|x| x.owner)
        } else {
            Some(winner)
        };
        world.kill_by(loser, crate::components::DeathCause::Violence, killer);
    }
    if !bout {
        let arms = crate::systems::weapons::fight_tiers(world, a, b);
        crate::systems::weapons::after_fight(world, winner, loser, died, arms);
        if !robot {
            crate::systems::fashion::after_fight(world, winner, loser);
            crate::systems::braindance::after_fight(world, winner);
        }
    }
    let side = |x: &Fighter, e: End| SideOut {
        id: x.id,
        stance: x.stance,
        kind: x.kind,
        end: e,
        took: x.took.clone(),
        landed: x.landed,
        dodged: x.dodged,
        chrome: [
            (x.gorilla && x.landed > 0).then_some("gorilla arms"),
            x.sandy_fired.then_some("Sandevistan"),
            (x.keren_used > 0).then_some("a Kerenzikov dodge"),
        ]
        .into_iter()
        .flatten()
        .collect(),
    };
    let ends = if li == 0 { [end, End::Won] } else { [End::Won, end] };
    let out =
        FightOutcome { sides: [side(&f[0], ends[0]), side(&f[1], ends[1])], winner, loser, died, beats, ticks, crowd };
    let text = line(world, &out, notes);
    world.push_event(EventKind::Fight, &[a, b], text);
    // C5 (`OPEN-295`): a knockout lasts; the winner may rob the body.
    if end == End::KnockedOut && !died && !f[li].robot {
        knock_out(world, loser, Some(winner), mods);
    }
    // C3b: a robot beaten in the beats is wrecked (Parts at the door).
    if end == End::Wrecked {
        crate::systems::assets::wreck(world, loser, "wrecked in a fight");
        crate::systems::ownership::jobs2_row(world).robots_wrecked += 1;
    }
    out
}

/// A fight with a robot (C1's `resolve_robot_fight` tail): the human's
/// memories name no robot; drift, safety and mood on the loser.
fn robot_sequel(world: &mut World, winner: EntityId, loser: EntityId) {
    if world.has::<Brain>(loser) {
        world.remember(loser, MemoryKind::Fought, None, 0.7, -0.5, false);
        world.remember(loser, MemoryKind::Lost, None, 0.6, -0.6, false);
        if let Some(p) = world.comp_mut::<Personality>(loser) {
            p.drift(Drift::LostFight);
        }
        if let Some(n) = world.comp_mut::<Needs>(loser) {
            n.safety = (n.safety - 0.4).max(0.0);
        }
        if let Some(m) = world.comp_mut::<crate::components::Mood>(loser) {
            m.value = (m.value - 0.3).max(-1.0);
        }
    }
    if world.has::<Brain>(winner) {
        world.remember(winner, MemoryKind::Fought, None, 0.7, -0.5, false);
        world.remember(winner, MemoryKind::Won, None, 0.6, 0.4, false);
        if let Some(p) = world.comp_mut::<Personality>(winner) {
            p.drift(Drift::WonFight);
        }
    }
}

/// A fight's tail on the two (not a bout's): the memories, drift, safety
/// and mood, the social edges.
fn sequel(
    world: &mut World,
    a: EntityId,
    b: EntityId,
    winner: EntityId,
    loser: EntityId,
    mods: &crate::systems::law::FightMods,
) {
    world.remember(a, MemoryKind::Fought, Some(b), 0.7, -0.5, false);
    world.remember(b, MemoryKind::Fought, Some(a), 0.7, -0.5, false);
    world.remember(winner, MemoryKind::Won, Some(loser), 0.6, 0.4, false);
    // Shadow V4 review 4: taken in by the law, the loser was arrested.
    let lost = if mods.arrest && loser == b { MemoryKind::WasArrested } else { MemoryKind::Lost };
    // The pressure pass R4: the attacker who lost carries himself as `object`.
    let object = (loser == a).then_some(a);
    let entry = crate::components::MemoryEntry {
        subject: Some(winner),
        salience: 0.6,
        valence: -0.6,
        object,
        ..crate::components::MemoryEntry::blank(lost, world.tick)
    };
    world.remember_entry(loser, entry);
    if let Some(p) = world.comp_mut::<Personality>(winner) {
        p.drift(Drift::WonFight);
    }
    if let Some(p) = world.comp_mut::<Personality>(loser) {
        p.drift(Drift::LostFight);
    }
    if let Some(n) = world.comp_mut::<Needs>(loser) {
        n.safety = (n.safety - 0.4).max(0.0);
    }
    if let Some(m) = world.comp_mut::<crate::components::Mood>(loser) {
        m.value = (m.value - 0.3).max(-1.0);
    }
    crate::systems::social::fought(world, winner, loser);
}

/// Down by blood loss (the M16b state); at `wounded_quota` (or with
/// `[wounded]` off) the body dies instead (`wounds_capped`). True: down.
pub(super) fn go_down(world: &mut World, id: EntityId, by: Option<EntityId>, murder: bool) -> bool {
    use crate::systems::wounded;
    if !wounded::on(world) || !world.has::<Identity>(id) || !world.has::<Brain>(id) {
        return false;
    }
    if wounded::is_down(world, id) {
        return true;
    }
    if world.wounded.values().filter(|w| !w.stable).count() >= world.config.wounded.wounded_quota {
        world.stats.current.wounded.wounds_capped += 1;
        return false;
    }
    wounded::down(world, id, by);
    if let Some(x) = world.wounded.get_mut(&id) {
        x.murder = murder && by.is_some();
    }
    true
}

/// The fight told in one line for the diaries: who, how (stance, weapon,
/// a limp carried in), the wounds by region, the end.
pub(super) fn line(world: &World, out: &FightOutcome, notes: [Option<&'static str>; 2]) -> String {
    let who = |s: &SideOut, note: Option<&str>| {
        let w = kind_row(world, s.kind).filter(|_| s.kind > 0).map_or("bare hands".to_string(), |r| r.id.clone());
        let n = note.map_or(String::new(), |n| format!(", {n}"));
        let ch = if s.chrome.is_empty() { String::new() } else { format!(", {}", s.chrome.join(", ")) };
        format!("{} ({}, {w}{ch}{n})", world.name_of(s.id), s.stance.label())
    };
    let mut text =
        format!("{} fought {}, {} beats", who(&out.sides[0], notes[0]), who(&out.sides[1], notes[1]), out.beats);
    // C3a: a live duel that ran across ticks says how long.
    if out.ticks > 0 {
        text.push_str(&format!(" over {} min", out.ticks + 1));
    }
    // C3b: the numbers ("3 more struck at X, 4 hits").
    for k in 0..2 {
        let (n, hits) = out.crowd[k];
        if n > 0 {
            let on = world.name_of(out.sides[1 - k].id);
            let more = if n == 1 { "1 more".to_string() } else { format!("{n} more") };
            text.push_str(&format!("; {more} struck at {on} with {}, {hits} hits", world.name_of(out.sides[k].id)));
        }
    }
    for s in &out.sides {
        if s.took.is_empty() {
            continue;
        }
        // Folded: "body grazed x3", worst first.
        let mut seen: Vec<((Region, Severity), u32)> = Vec::new();
        for &x in &s.took {
            match seen.iter_mut().find(|(k, _)| *k == x) {
                Some((_, n)) => *n += 1,
                None => seen.push((x, 1)),
            }
        }
        seen.sort_by_key(|p| std::cmp::Reverse(p.0 .1));
        // C4 (C3b's wrong-read): a robot is dented and smashed, not wounded.
        let robot = crate::systems::robots::is_robot(world, s.id);
        let hurts: Vec<String> = seen
            .iter()
            .map(|&((r, v), n)| {
                let (r, v) = if robot { (robot_part(r), robot_harm(v)) } else { (r.label(), v.label()) };
                if n > 1 {
                    format!("{r} {v} x{n}")
                } else {
                    format!("{r} {v}")
                }
            })
            .collect();
        text.push_str(&format!("; {} hit: {}", world.name_of(s.id), hurts.join(", ")));
    }
    let l = if out.sides[0].id == out.loser { &out.sides[0] } else { &out.sides[1] };
    text.push_str(&format!("; {} {}", world.name_of(l.id), l.end.label()));
    text
}
