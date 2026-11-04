use super::*;

// ---------------------------------------------------------------------------
// C4: the odds (mock fights on the beat's rules, no world writes)
// ---------------------------------------------------------------------------

/// C4: an Attack's odds: side 0 (the attacker's) wins on `p`; the heads
/// each side can expect (the likely joiners at their join chance).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Odds {
    pub p: f32,
    pub mine: f32,
    pub theirs: f32,
}

pub(super) struct Mock {
    f: Fighter,
    blood: f32,
}

/// The odds that `a` beats `b`: in their fight, its sides as they stand (`b`
/// stepped up to face him); else `a` attacking `b` (an Attack: the law
/// joins `b`), with the likely joiners within `join_tiles` at their chance.
pub fn odds(world: &World, a: EntityId, b: EntityId) -> Odds {
    let c = &world.config.combat;
    let mods = crate::systems::law::FightMods::default();
    let mk = |x: EntityId, k: usize, live: Option<&Fighter>| -> Fighter {
        live.cloned().unwrap_or_else(|| {
            let st = stance_for(world, x, k == 0, &mods);
            let mut f = fighter(world, x, st, k == 0, 0.0, false);
            f.attacked |= k == 1;
            f
        })
    };
    let mut sides: [Vec<(Fighter, f32)>; 2] = [Vec::new(), Vec::new()];
    let eng = world.combat.engaged.get(&a).and_then(|i| world.combat.engagements.get(i));
    match eng.filter(|e| e.members().any(|x| x == b)) {
        Some(e) => {
            let ka = usize::from(!e.sides[0].contains(&a));
            let live = |x: EntityId| {
                e.duel.as_ref().and_then(|d| d.f.iter().chain(d.aid.iter().flatten()).find(|f| f.id == x))
            };
            for (k, side) in [(0, &e.sides[ka]), (1, &e.sides[1 - ka])] {
                let mut ids: Vec<EntityId> = side.clone();
                let head = if k == 1 { b } else { a };
                ids.retain(|&y| y != head);
                ids.insert(0, head);
                sides[k] = ids.iter().map(|&x| (mk(x, k, live(x)), 1.0)).collect();
            }
        }
        None => {
            sides[0].push((mk(a, 0, None), 1.0));
            sides[1].push((mk(b, 1, None), 1.0));
            if let Some(p) = world.comp::<Position>(b) {
                let ids = [vec![a], vec![b]];
                let ctx = JoinCtx::of(world, &ids, &mods, &Tail::Bare, p.tile, p.building, false);
                let r = i32::from(c.join_tiles as u16);
                for x in world.entities() {
                    if x == a || x == b || !world.has::<Brain>(x) || engaged(world, x) {
                        continue;
                    }
                    let near = world.comp::<Position>(x).is_some_and(|q| match (q.building, p.building) {
                        (Some(u), Some(v)) => u == v,
                        (None, None) => {
                            (i32::from(q.tile.x) - i32::from(p.tile.x)).abs() <= r
                                && (i32::from(q.tile.y) - i32::from(p.tile.y)).abs() <= r
                        }
                        _ => false,
                    });
                    if !near {
                        continue;
                    }
                    if let Some((k, pj, _)) = ctx.check(world, &ids, x) {
                        if sides[0].len() + sides[1].len() < 2 + c.join_max {
                            sides[k].push((mk(x, k, None), pj.clamp(0.0, 1.0)));
                        }
                    }
                }
            }
        }
    }
    let heads = |k: usize| sides[k].iter().map(|s| s.1).sum::<f32>();
    let (mine, theirs) = (heads(0), heads(1));
    let salt = key(a) ^ key(b).rotate_left(29) ^ (world.tick / TICKS_PER_HOUR);
    Odds { p: mock(world, &sides, salt), mine, theirs }
}

/// C5: the odds of the duel `a` would fight with `b` alone (as they stand
/// in their fight, else fresh): the chain of duels' one link, beside the
/// whole fight's [`odds`].
pub fn duel_odds(world: &World, a: EntityId, b: EntityId) -> f32 {
    let mods = crate::systems::law::FightMods::default();
    let eng = world.combat.engaged.get(&a).and_then(|i| world.combat.engagements.get(i));
    let live = |x: EntityId| {
        eng.and_then(|e| e.duel.as_ref()).and_then(|d| d.f.iter().chain(d.aid.iter().flatten()).find(|f| f.id == x))
    };
    let mk = |x: EntityId, k: usize| -> Fighter {
        let mut f = live(x).cloned().unwrap_or_else(|| {
            let st = stance_for(world, x, k == 0, &mods);
            fighter(world, x, st, k == 0, 0.0, false)
        });
        f.attacked |= k == 1;
        f.end = None;
        f
    };
    let sides = [vec![(mk(a, 0), 1.0)], vec![(mk(b, 1), 1.0)]];
    let salt = key(a) ^ key(b).rotate_left(31) ^ (world.tick / TICKS_PER_HOUR);
    mock(world, &sides, salt)
}

/// C4: `odds_trials` mock fights of `sides` (each member present on its
/// chance): the share side 0 wins. The beat's rules (the break and its
/// numbers, the stances, the split defence, the crowd's reach, the hit,
/// region and severity, armour, chrome limbs, a knockout, a kill, the blood)
/// without fatigue, the Kerenzikov or the Sandevistan.
fn mock(world: &World, sides: &[Vec<(Fighter, f32)>; 2], salt: u64) -> f32 {
    let c = &world.config.combat;
    let trials = c.odds_trials.max(1);
    let reach = c.crowd_reach.max(1);
    let blood0 = |x: EntityId| world.comp::<Wounds>(x).map_or(1.0, |w| w.blood);
    let mut won = 0u32;
    for t in 0..trials {
        let mut n = 0u64;
        let mut roll = || {
            n += 1;
            hash(world, PURPOSE_ODDS, salt ^ u64::from(t).rotate_left(40), n)
        };
        let mut s: [Vec<Mock>; 2] = [Vec::new(), Vec::new()];
        for k in 0..2 {
            for (f, p) in &sides[k] {
                if *p >= 1.0 || roll() < *p {
                    s[k].push(Mock { f: f.clone(), blood: blood0(f.id) });
                }
            }
        }
        let winner = loop {
            let fa = s[0].iter().position(|m| m.f.end.is_none());
            let fb = s[1].iter().position(|m| m.f.end.is_none());
            let (Some(fa), Some(fb)) = (fa, fb) else { break usize::from(fb.is_some()) };
            let front = [fa, fb];
            let max_beats = beat_cap(c, s[0][fa].f.human || s[1][fb].f.human);
            let mut beat = 0;
            loop {
                if s[0][fa].f.end.is_some() || s[1][fb].f.end.is_some() {
                    break;
                }
                if beat >= max_beats {
                    let i = usize::from(s[0][fa].f.morale >= s[1][fb].f.morale);
                    s[i][front[i]].f.end = Some(End::Yielded);
                    break;
                }
                beat += 1;
                for side in s.iter_mut() {
                    for m in side.iter_mut() {
                        m.f.faced = 0;
                    }
                }
                let heads = [0, 1].map(|k| s[k].iter().filter(|m| m.f.end.is_none()).count().max(1) as f32);
                let order = if s[1][fb].f.reflex > s[0][fa].f.reflex { [1, 0] } else { [0, 1] };
                for i in order {
                    let j = 1 - i;
                    if s[i][front[i]].f.end.is_some() || s[j][front[j]].f.end.is_some() {
                        break;
                    }
                    let m = &s[i][front[i]].f;
                    let morale = m.morale + c.numbers_morale * (heads[i] / heads[j]).ln();
                    let line = c.break_at + c.break_shift[m.stance.i()];
                    // C5: the human's body shakes; the break needs a blow or the numbers
                    if m.human {
                        s[i][front[i]].f.shake = (line - morale).max(0.0);
                    } else if ((m.struck && m.swung) || heads[j] > heads[i]) && morale < line {
                        let fx = crate::systems::health::effects(world, m.id);
                        let p = (c.flee_p + c.flee_evade_w * m.evasion) * fx.evade_mult;
                        let fled = !lame(world, m.id) && roll() < p;
                        if fled || s[j][front[j]].f.stance.takes_yield(false) {
                            s[i][front[i]].f.end = Some(End::Fled);
                            break;
                        }
                    }
                    if s[i][front[i]].f.stance == Stance::HoldBack && !s[i][front[i]].f.attacked {
                        continue;
                    }
                    let (att, tgt) = pair(&mut s, i, front[i], front[j]);
                    mock_strike(world, &mut att.f, tgt, &mut roll);
                    att.f.swung = true;
                }
                for k in 0..2 {
                    let mut melee = 1;
                    for h in 0..s[k].len() {
                        if h == front[k] || s[k][h].f.end.is_some() {
                            continue;
                        }
                        if s[1 - k][front[1 - k]].f.end.is_some() {
                            break;
                        }
                        if !ranged(world, s[k][h].f.kind) {
                            if melee >= reach {
                                continue;
                            }
                            melee += 1;
                        }
                        let (att, tgt) = pair(&mut s, k, h, front[1 - k]);
                        mock_strike(world, &mut att.f, tgt, &mut roll);
                    }
                }
            }
        };
        won += u32::from(winner == 0);
    }
    won as f32 / trials as f32
}

/// The mock fighter `h` of side `k` and the other side's `t`.
pub(super) fn pair(s: &mut [Vec<Mock>; 2], k: usize, h: usize, t: usize) -> (&mut Mock, &mut Mock) {
    let [s0, s1] = s;
    if k == 0 {
        (&mut s0[h], &mut s1[t])
    } else {
        (&mut s1[h], &mut s0[t])
    }
}

/// [`strike`]'s rules on a mock target (no world writes).
fn mock_strike(world: &World, att: &mut Fighter, tgt: &mut Mock, roll: &mut impl FnMut() -> f32) {
    let c = &world.config.combat;
    let Some(row) = kind_row(world, att.kind) else { return };
    let (mut dmg, mut lethality) = (row.damage, row.lethality);
    if att.gorilla {
        dmg[2] *= c.gorilla_grave;
        lethality = lethality.max(c.gorilla_lethality);
    }
    let t = &mut tgt.f;
    t.attacked = true;
    t.faced = t.faced.saturating_add(1);
    let split = (1.0 - c.split_evade * f32::from(t.faced - 1)).max(0.0);
    let evade = (t.evasion + c.stance_evade[t.stance.i()]) * split * shaken(c, c.shake_evade, t.shake);
    let attack = att.attack * shaken(c, c.shake_attack, att.shake);
    let p = (c.hit_base + c.attack_w * attack + row.accuracy * att.quality + c.stance_attack[att.stance.i()]
        - c.evade_w * evade)
        .clamp(c.hit_min, c.hit_max);
    let p = if row.family == WeaponFamily::Ranged { p * (1.0 - t.cover) } else { p };
    if roll() >= p {
        return;
    }
    t.struck = true;
    att.morale += c.morale_landed;
    let region = Region::ALL[pick_weighted(roll(), &c.region_weights)];
    let q = att.quality.max(0.01);
    dmg[0] /= q;
    dmg[2] *= q;
    if row.family == WeaponFamily::Melee {
        dmg[2] *= 1.0 + c.strength_grave * att.strength;
    }
    let mut sev = [Severity::Graze, Severity::Wound, Severity::Grave][pick_weighted(roll(), &dmg)];
    if t.robot {
        t.wreck = t.wreck.saturating_add(match sev {
            Severity::Graze => 0,
            Severity::Wound => 1,
            Severity::Grave => c.robot_wreck_hits,
        });
        if t.wreck >= c.robot_wreck_hits.max(1) {
            t.end = Some(End::Wrecked);
        }
        return;
    }
    let armour = if region == Region::Torso && t.vest { t.skin.max(c.vest_torso) } else { t.skin };
    if armour > 0.0 && roll() < armour * c.armour_w {
        match sev.down() {
            Some(x) => sev = x,
            None => return,
        }
    }
    if (region.is_arm() && t.chrome_arms.is_some()) || (region.is_leg() && t.chrome_legs.is_some()) {
        t.morale -= c.morale_hit[sev.index()] * 0.5;
        return;
    }
    let subdue = att.stance == Stance::Subdue;
    let mut ko = false;
    if subdue && sev == Severity::Grave {
        sev = Severity::Wound;
        ko = true;
    }
    if subdue && !ko {
        ko = roll() < c.subdue_ko_p * (1.0 + t.fatigue);
    }
    if sev == Severity::Grave && matches!(region, Region::Head | Region::Torso) && roll() < lethality * q {
        t.end = Some(End::Dead);
        return;
    }
    if !ko && region == Region::Head {
        let m = if row.blunt || att.gorilla { c.blunt_ko_mult } else { 1.0 };
        ko = roll() < c.ko_p[sev.index()] * m;
    }
    t.morale -= c.morale_hit[sev.index()];
    if sev >= Severity::Wound {
        if region.is_arm() {
            t.attack *= c.arm_mult[sev.index()];
        } else if region.is_leg() {
            t.evasion *= c.leg_mult[sev.index()];
        }
    }
    if !row.blunt {
        tgt.blood -= c.blood_hit[sev.index()];
    }
    if tgt.blood <= 0.0 {
        tgt.f.end = Some(End::Downed);
    } else if ko {
        tgt.f.end = Some(End::KnockedOut);
    }
}
