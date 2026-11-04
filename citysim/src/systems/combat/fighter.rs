use super::*;

// ---------------------------------------------------------------------------
// Weapon kinds
// ---------------------------------------------------------------------------

/// The kinds table (row 0 the bare hand).
fn kinds(world: &World) -> &[WeaponKindCfg] {
    &world.config.weapon_kinds
}

pub(super) fn kind_row(world: &World, k: usize) -> Option<&WeaponKindCfg> {
    kinds(world).get(k)
}

/// The class a weapon's holder or place maps the stock by: `law` (a guard,
/// the Precinct's stock), `gang` (a gang, a member, an armoury), else `civil`.
pub fn class_of(world: &World, owner: Option<EntityId>, loc: AssetLoc) -> &'static str {
    let holder = match loc {
        AssetLoc::Carried(h) | AssetLoc::Parked(h) | AssetLoc::Stock(h) => Some(h),
        _ => None,
    };
    for x in [owner, holder].into_iter().flatten() {
        if crate::systems::law::is_guard(world, x)
            || world
                .comp::<crate::components::Building>(x)
                .is_some_and(|b| b.kind == crate::components::BuildingKind::Jail)
        {
            return "law";
        }
        if world.has::<Gang>(x) || world.gang_of(x).is_some() {
            return "gang";
        }
    }
    "civil"
}

/// The kind row (+ 1) a weapon of `tier` held by `class` maps to: a keyed
/// pick among the rows listing both, else the rows listing the tier, else
/// the first armed row.
pub fn map_kind(world: &World, tier: u8, class: &str, salt: u64) -> u8 {
    let rows = kinds(world);
    let pick = |f: &dyn Fn(&WeaponKindCfg) -> bool| -> Vec<usize> {
        rows.iter().enumerate().skip(1).filter(|(_, r)| f(r)).map(|(i, _)| i).collect()
    };
    let mut c = pick(&|r| r.tiers.contains(&tier) && r.classes.iter().any(|x| x == class));
    if c.is_empty() {
        c = pick(&|r| r.tiers.contains(&tier));
    }
    if c.is_empty() {
        return u8::from(rows.len() > 1) * 2;
    }
    let i = ((hash(world, PURPOSE_KIND, salt, u64::from(tier)) * c.len() as f32) as usize).min(c.len() - 1);
    (c[i] + 1).min(255) as u8
}

/// The kind `id` fights with: its carried weapon's (`Kit.weapon_kind`), a
/// guard's draw from the armoury (the law's map for its tier), else the
/// bare hand. 0 with `[weapons]` off.
pub fn kind_of(world: &World, id: EntityId) -> usize {
    let t = crate::systems::weapons::tier(world, id);
    if t == 0 {
        return 0;
    }
    let own = world.comp::<Kit>(id).filter(|k| k.weapon == t && k.weapon_kind > 0).map(|k| usize::from(k.weapon_kind));
    own.unwrap_or_else(|| usize::from(map_kind(world, t, "law", key(id))).saturating_sub(1))
}

pub(super) fn ranged(world: &World, k: usize) -> bool {
    k > 0 && kind_row(world, k).is_some_and(|r| r.family == WeaponFamily::Ranged)
}

/// A kind's concealment term (`weapons::notice_p`), `tier - 1` without a row.
pub fn conceal_of(world: &World, id: EntityId, tier: u8) -> f32 {
    let k = world.comp::<Kit>(id).map_or(0, |k| usize::from(k.weapon_kind));
    match kind_row(world, k).filter(|_| k > 0) {
        Some(r) => r.conceal,
        None => f32::from(tier.max(1)) - 1.0,
    }
}

// ---------------------------------------------------------------------------
// Skills and power
// ---------------------------------------------------------------------------

/// A body's fighting power on its skills alone (the city's skill means and
/// competence read it): the better of melee and shooting, plus
/// `power_evasion` of evasion.
pub fn skill_power(world: &World, s: &Skills) -> f32 {
    s.melee.max(s.shooting.max(0.0)) + world.config.combat.power_evasion * s.evasion.max(0.0)
}

/// `combat::power(world, id)` (docs/COMBAT.md § 3): what every gate reads
/// (the odds in `moves`, the contracts' ability gates, the Hit's quote, a
/// raid's pairing): the better of melee (with the Arms' `Kit.fighting`) and
/// shooting with a gun carried, x the arms' wounds, plus a share of
/// evasion x the legs'. A robot fights at `robot_fighting[tier]`; a body
/// jacked in at `jacked_fight_mult`.
pub fn power(world: &World, id: EntityId) -> f32 {
    let Some(s) = world.comp::<Skills>(id) else {
        return crate::systems::robots::fighting(world, id).unwrap_or(0.2);
    };
    let kit = world.comp::<Kit>(id);
    let melee = s.melee + kit.map_or(0.0, |k| k.fighting);
    let shoot = if kit.is_some_and(|k| k.weapon > 0) || crate::systems::law::is_guard(world, id) {
        if ranged(world, kind_of(world, id)) {
            s.shooting.max(0.0)
        } else {
            0.0
        }
    } else {
        0.0
    };
    let fx = crate::systems::health::effects(world, id);
    let f = melee.max(shoot) * fx.attack_mult + world.config.combat.power_evasion * s.evasion.max(0.0) * fx.evade_mult;
    if !world.runner_of.is_empty() && world.runner_of.contains_key(&id) {
        return f * world.config.virt.jacked_fight_mult;
    }
    f
}

/// A role's config key: a `[[trades]]` row's id, else the `Role` name.
pub fn role_key(role: crate::components::Role) -> String {
    match role {
        crate::components::Role::Trade(t) => t.key().to_string(),
        r => format!("{r:?}"),
    }
}

/// `id`'s shooting seed: its post's base (a `shooting_posts` role, else a
/// gang member's `shooting_gang`, else `shooting_base`) x its keyed draw;
/// `post`: the base was a post's (a role or a gang), not a civilian's.
fn shooting_seed(world: &World, id: EntityId) -> (f32, bool) {
    let c = &world.config.combat;
    let role = world.comp::<Job>(id).map(|j| role_key(j.role));
    let (base, post) = match role.and_then(|r| c.shooting_posts.get(&r).copied()) {
        Some(b) => (b, true),
        None if world.gang_of(id).is_some() => (c.shooting_gang, true),
        None => (c.shooting_base, false),
    };
    let [lo, hi] = c.shooting_draw;
    (base * (lo + (hi - lo) * hash(world, PURPOSE_SHOOT, key(id), 0)), post)
}

/// Seed `id`'s shooting and evasion when unset (-1): shooting from its
/// role's base x a keyed draw, evasion a keyed draw x the age curve.
pub fn seed_skills(world: &mut World, id: EntityId) {
    let Some(s) = world.comp::<Skills>(id) else { return };
    if s.shooting >= 0.0 && s.evasion >= 0.0 {
        return;
    }
    let c = &world.config.combat;
    let shoot = shooting_seed(world, id).0;
    let (slo, shi) = (world.config.world.skill_min, world.config.world.skill_max);
    let years = world.comp::<Identity>(id).map_or(30.0, |i| i.age_days as f32 / crate::time::DAYS_PER_YEAR as f32);
    let age = (1.0 - c.evasion_decline * (years - c.evasion_peak).max(0.0)).max(c.evasion_floor);
    let evade = (slo + (shi - slo) * hash(world, PURPOSE_EVADE, key(id), 0)) * age;
    if let Some(s) = world.comp_mut::<Skills>(id) {
        if s.shooting < 0.0 {
            s.shooting = shoot.clamp(0.0, 1.0);
        }
        if s.evasion < 0.0 {
            s.evasion = evade.clamp(0.0, 1.0);
        }
    }
}

/// C1's wrong-reads (b, d), C2: a post's training. A body on a shooting
/// post or in a gang is brought up to its shooting seed (one who joined
/// after the seed shot like a civilian: raiders at 0.04), and a
/// `melee_posts` / `evasion_posts` role to those floors.
fn train(world: &mut World, id: EntityId) {
    let Some(s) = world.comp::<Skills>(id) else { return };
    let (melee, evasion, shooting) = (s.melee, s.evasion, s.shooting);
    if shooting < 0.0 {
        return;
    }
    let c = &world.config.combat;
    let role = world.comp::<Job>(id).map(|j| role_key(j.role));
    let floor = |m: &std::collections::BTreeMap<String, f32>| role.as_ref().and_then(|r| m.get(r).copied());
    let (fm, fe) = (floor(&c.melee_posts), floor(&c.evasion_posts));
    let (shoot, post) = shooting_seed(world, id);
    let up_shoot = post && shoot > shooting;
    let up_melee = fm.is_some_and(|f| f > melee);
    let up_evade = fe.is_some_and(|f| f > evasion);
    if !(up_shoot || up_melee || up_evade) {
        return;
    }
    if let Some(s) = world.comp_mut::<Skills>(id) {
        if up_shoot {
            s.shooting = shoot.clamp(0.0, 1.0);
        }
        if let Some(f) = fm.filter(|_| up_melee) {
            s.melee = f.min(1.0);
        }
        if let Some(f) = fe.filter(|_| up_evade) {
            s.evasion = f.min(1.0);
        }
    }
}

/// Every body with skills unset (`World::new`, and at midnight for the
/// day's arrivals and those come of age); C2, the posts' training.
pub fn seed_all(world: &mut World) {
    let ids: Vec<EntityId> = world.with::<Skills>();
    for id in ids {
        seed_skills(world, id);
        train(world, id);
    }
}

// ---------------------------------------------------------------------------
// Wound effects (read through `health::effects`)
// ---------------------------------------------------------------------------

/// The wounds' terms on `health::effects`: the arms x the attack, the legs
/// x evasion and the flight and the step cost, the head's knockout term and
/// x work.
pub fn apply_effects(world: &World, w: &Wounds, e: &mut crate::systems::health::Effects) {
    let c = &world.config.combat;
    for r in Region::ALL {
        let Some(sev) = w.worst(r) else { continue };
        let i = sev.index();
        match r {
            Region::ArmL | Region::ArmR => e.attack_mult *= c.arm_mult[i],
            Region::LegL | Region::LegR => {
                e.evade_mult *= c.leg_mult[i];
                e.walk_mult = e.walk_mult.max(c.leg_walk[i]);
            }
            Region::Head => {
                e.ko_add += c.head_ko[i];
                e.work_mult *= c.head_work[i];
            }
            Region::Torso => {}
        }
    }
}

/// What a body carries into a fight, told in the line ("limping", "a bad arm").
pub(super) fn carried_note(world: &World, id: EntityId) -> Option<&'static str> {
    let w = world.comp::<Wounds>(id)?;
    if w.marks.iter().any(|m| m.1 == Mark::Lost && m.0.is_arm()) {
        return Some("one-armed");
    }
    if w.marks.iter().any(|m| m.1 == Mark::Lost && m.0.is_leg()) {
        return Some("on one leg");
    }
    let leg = w.list.iter().any(|x| x.region.is_leg() && x.severity >= Severity::Wound);
    let arm = w.list.iter().any(|x| x.region.is_arm() && x.severity >= Severity::Wound);
    match (leg, arm) {
        (true, true) => Some("limping, a bad arm"),
        (true, false) => Some("limping"),
        (false, true) => Some("a bad arm"),
        _ if w.list.iter().any(|x| x.region == Region::Head && x.severity >= Severity::Wound) => Some("head bandaged"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The beat loop
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(super) struct Fighter {
    pub(super) id: EntityId,
    pub(super) stance: Stance,
    pub(super) kind: usize,
    pub(super) quality: f32,
    /// The skill its weapon uses, less frailty, plus the crew bonus.
    pub(super) attack: f32,
    pub(super) evasion: f32,
    pub(super) strength: f32,
    pub(super) reflex: f32,
    pub(super) morale: f32,
    /// C3b: its morale at the start, before any carry (the carry's base).
    pub(super) morale0: f32,
    pub(super) skin: f32,
    pub(super) vest: bool,
    pub(super) ready_at: u8,
    pub(super) attacked: bool,
    pub(super) cornered: bool,
    pub(super) robot: bool,
    pub(super) end: Option<End>,
    pub(super) took: Vec<(Region, Severity)>,
    pub(super) landed: u8,
    pub(super) dodged: u8,
    pub(super) melee_hits: u8,
    pub(super) shots: u8,
    /// C3b: the caller's crew bonus it fights with (kept across a wound's
    /// recompute), and whether it is on side 0 (the starter's).
    pub(super) bonus: f32,
    pub(super) side0: bool,
    /// Fatigue (0 fresh, 1 spent; `[combat] tire_*`) and its drain's
    /// multiplier (Kit strength); attacks faced this beat (the split defence).
    pub(super) fatigue: f32,
    pub(super) tire_mult: f32,
    pub(super) faced: u8,
    /// A Kerenzikov's dodge charges and its chance (0: none); `hurt_once`:
    /// the second charge has come.
    pub(super) keren: u8,
    pub(super) keren_p: f32,
    pub(super) hurt_once: bool,
    /// A Sandevistan's extra strikes a beat (0: none), the beats left of
    /// its use, and whether it may fire in this fight (off cooldown).
    pub(super) sandy: u8,
    pub(super) sandy_left: u8,
    pub(super) sandy_armed: bool,
    /// Gorilla arms: the bare hand hits like a club and kills.
    pub(super) gorilla: bool,
    /// The chrome limbs a hit there wrecks instead of wounding.
    pub(super) chrome_arms: Option<EntityId>,
    pub(super) chrome_legs: Option<EntityId>,
    /// A robot's damage (`robot_wreck_hits` wrecks it).
    pub(super) wreck: u8,
    /// The chrome that showed (for the line): Kerenzikov dodges, the
    /// Sandevistan fired, gorilla arms landed.
    pub(super) keren_used: u8,
    pub(super) sandy_fired: bool,
    /// C4: the cover where it stands (`cover_at`: a ranged hit x (1 - it)).
    pub(super) cover: f32,
    /// C4: the human's order to break off (flee, yield), tried at its turn.
    pub(super) want: Option<Want>,
    /// C5: the human drives it: its morale break is waived (the human
    /// decides when to break); under the line it shakes by `shake` (how far
    /// under its break line, with the numbers).
    pub(super) human: bool,
    pub(super) shake: f32,
    /// C5 (`OPEN-296`): a blow has landed on it in this engagement, and it
    /// has swung (its turn's strike): the break needs that exchange, or the
    /// numbers against it.
    pub(super) struck: bool,
    pub(super) swung: bool,
}

/// C3b: what a body carries from one duel of an engagement to the next
/// (morale lost or won, the Kerenzikov's charges, the Sandevistan's beats).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Carry {
    pub(super) morale: f32,
    pub(super) keren: u8,
    pub(super) hurt_once: bool,
    pub(super) sandy_left: u8,
    pub(super) sandy_used: bool,
    pub(super) struck: bool,
    pub(super) swung: bool,
}

/// A working implant in `slot` and its effective tier.
pub(super) fn chrome_tier(world: &World, id: EntityId, slot: Slot) -> Option<(EntityId, u8)> {
    world.comp::<Skills>(id)?;
    let a = crate::systems::chrome::implant_in(world, id, slot)?;
    let x = world.comp::<crate::components::Asset>(a).filter(|x| x.condition > 0 && !x.bricked)?;
    Some((a, crate::systems::assets::eff_tier_of(world, x).max(1)))
}

/// `id`'s fatigue now: the last fight's, less `rest_hour` an hour since.
pub(super) fn fatigue_of(world: &World, id: EntityId) -> f32 {
    world.combat.tired.get(&id).map_or(0.0, |&(f, at)| {
        let hours = world.tick.saturating_sub(at) as f32 / TICKS_PER_HOUR as f32;
        (f - world.config.combat.rest_hour * hours).max(0.0)
    })
}

/// The stance `id` takes as the starter (`attacker`) or the one set upon.
pub(super) fn stance_for(world: &World, id: EntityId, attacker: bool, mods: &crate::systems::law::FightMods) -> Stance {
    if crate::systems::robots::is_robot(world, id) {
        return Stance::AllOut;
    }
    if mods.why == Purpose::Bout {
        return Stance::Subdue;
    }
    // C4: a body the human drives takes the human's stance.
    if let Some(s) = orders_of(world, id).and_then(|o| o.stance) {
        return s;
    }
    let p = world.comp::<Personality>(id);
    let (courage, lawful) = p.map_or((0.5, 0.5), |p| (p.courage, p.lawfulness));
    if attacker {
        if mods.arrest {
            return Stance::Subdue;
        }
        if mods.kill_p.is_some() || mods.kill_mult > 1.0 || mods.why == Purpose::Raid {
            return Stance::AllOut;
        }
        if courage < 0.3 {
            return Stance::HoldBack;
        }
        return if lawful < 0.4 { Stance::AllOut } else { Stance::Defensive };
    }
    if mods.why == Purpose::Door || (crate::systems::law::is_guard(world, id) && !mods.arrest) {
        return Stance::Subdue;
    }
    if mods.why == Purpose::Raid || (mods.arrest && mods.kill_mult > 1.0) {
        return Stance::AllOut;
    }
    if courage < 0.35 {
        Stance::HoldBack
    } else if courage > 0.7 && lawful < 0.4 {
        Stance::AllOut
    } else {
        Stance::Defensive
    }
}

/// The years `id` is past `[combat] age_from` (0 for the young and robots).
pub(super) fn over_age(world: &World, id: EntityId) -> f32 {
    world.comp::<Identity>(id).map_or(0.0, |i| {
        (i.age_days as f32 / crate::time::DAYS_PER_YEAR as f32 - world.config.combat.age_from).max(0.0)
    })
}

pub(super) fn fighter(world: &World, id: EntityId, stance: Stance, attacker: bool, bonus: f32, bout: bool) -> Fighter {
    let c = &world.config.combat;
    let robot = crate::systems::robots::is_robot(world, id);
    let mut kind = if bout { 0 } else { kind_of(world, id) };
    let mut tier = crate::systems::weapons::tier(world, id);
    // C3b: a robot fights with its tier's kind (`robot_kinds`).
    if robot {
        let t = world.comp::<crate::components::Asset>(id).map_or(1, |x| crate::systems::assets::eff_tier_of(world, x));
        let t = t.clamp(1, 3);
        let want = &c.robot_kinds[usize::from(t - 1)];
        kind = kinds(world).iter().position(|r| &r.id == want).unwrap_or(0);
        tier = t;
    }
    // C4: the human holstered the weapon (bare hands, the blades sheathed).
    let ord = orders_of(world, id);
    let holster = ord.is_some_and(|o| o.holster);
    if holster {
        kind = 0;
    }
    // Subdue strikes blunt: the first blunt armed row when armed.
    if stance == Stance::Subdue && kind > 0 && !kind_row(world, kind).is_some_and(|r| r.blunt) {
        kind = kinds(world).iter().enumerate().skip(1).find(|(_, r)| r.blunt).map_or(0, |(i, _)| i);
    }
    let quality = if kind > 0 { c.quality[usize::from(tier.clamp(1, 3) - 1)] } else { 1.0 };
    let kit = world.comp::<Kit>(id);
    let fx = crate::systems::health::effects(world, id);
    let (skill, evasion) = match world.comp::<Skills>(id) {
        Some(s) => {
            let sk = if ranged(world, kind) { s.shooting.max(0.0) } else { s.melee + kit.map_or(0.0, |k| k.fighting) };
            (sk, s.evasion.max(0.0))
        }
        None => {
            let r = crate::systems::robots::fighting(world, id).unwrap_or(0.2);
            (r, r)
        }
    };
    let jacked = !world.runner_of.is_empty() && world.runner_of.contains_key(&id);
    let jm = if jacked { world.config.virt.jacked_fight_mult } else { 1.0 };
    let reflex = kit.map_or(0.0, |k| k.reflex);
    // C2, C1's wrong-read (a): age slows the fist, the dodge and the nerve.
    let over = over_age(world, id);
    let age = (1.0 - c.age_fight * over).max(c.age_fight_floor);
    let attack = (skill * fx.attack_mult * age - fx.frail_fight) * jm + bonus;
    let evasion = (evasion + c.reflex_evade * reflex) * fx.evade_mult * age * jm;
    let courage = crate::systems::law::courage(world, id);
    // C3a: a berserker in its episode never breaks (it fights the cuffs to
    // the end, M13 D33), as a robot does not.
    let morale = if robot || crate::systems::chrome::in_episode(world, id) {
        f32::INFINITY
    } else {
        // C2 (d): a trained post holds its nerve.
        let post = if c.morale_posts.is_empty() {
            0.0
        } else {
            world.comp::<Job>(id).and_then(|j| c.morale_posts.get(&role_key(j.role)).copied()).unwrap_or(0.0)
        };
        c.morale_base + c.courage_w * (courage - 0.5) + c.stance_morale[stance.i()] + bonus - c.age_morale * over + post
    };
    // C3b, chrome (§ 4): the Arms' blades or gorilla arms, the Nerves'
    // Kerenzikov and Sandevistan, the limbs a hit wrecks.
    let arms = chrome_tier(world, id, Slot::Arms);
    let mut quality = quality;
    if let Some((_, t)) = arms.filter(|&(_, t)| t >= c.mantis_tier && stance != Stance::Subdue && !bout && !holster) {
        if !ranged(world, kind) {
            if let Some(m) = kinds(world).iter().position(|r| r.id == c.mantis_kind) {
                kind = m;
                quality = c.quality[usize::from(t.clamp(1, 3) - 1)];
            }
        }
    }
    let gorilla = arms.is_some() && kind == 0;
    let nerves = chrome_tier(world, id, Slot::Nerves);
    let keren_p = nerves.map_or(0.0, |(_, t)| c.keren_p[usize::from(t.clamp(1, 3) - 1)]);
    let sandy =
        nerves.filter(|&(_, t)| t >= c.sandy_tier).map_or(0, |(_, t)| c.sandy_actions[usize::from(t.clamp(1, 3) - 1)]);
    let sandy_armed = sandy > 0 && world.combat.sandy_cd.get(&id).is_none_or(|&t| t <= world.tick);
    let strength = kit.map_or(0.0, |k| k.strength);
    let draw = if attacker { 0 } else { kind_row(world, kind).map_or(0, |r| r.draw) };
    Fighter {
        id,
        stance,
        kind,
        quality,
        attack,
        evasion,
        strength,
        reflex,
        morale,
        morale0: morale,
        skin: kit.map_or(0.0, |k| k.armour),
        vest: crate::systems::law::is_guard(world, id),
        ready_at: draw,
        attacked: attacker,
        cornered: false,
        robot,
        end: None,
        took: Vec::new(),
        landed: 0,
        dodged: 0,
        melee_hits: 0,
        shots: 0,
        bonus,
        side0: attacker,
        fatigue: if robot { 0.0 } else { fatigue_of(world, id) },
        tire_mult: if robot { 0.0 } else { (1.0 - c.tire_strength * strength).max(0.2) },
        faced: 0,
        keren: u8::from(keren_p > 0.0),
        keren_p,
        hurt_once: false,
        sandy,
        sandy_left: 0,
        sandy_armed,
        gorilla,
        chrome_arms: arms.map(|a| a.0),
        chrome_legs: chrome_tier(world, id, Slot::Legs).map(|a| a.0),
        wreck: 0,
        keren_used: 0,
        sandy_fired: false,
        cover: world.comp::<Position>(id).map_or(0.0, |p| cover_at(world, p.tile, p.building)),
        // M18 P5 (Dylan 2026-11-03): the player's body on autopilot breaks
        // off at its first turn (a flight, else a yield).
        want: ord
            .and_then(|o| o.want)
            .or_else(|| (!robot && crate::systems::player::safe_idle(world, id)).then_some(Want::Flee)),
        human: !robot && crate::systems::player::drives(world, id),
        shake: 0.0,
        struck: false,
        swung: false,
    }
}

/// C5: a shaking body's attack or evasion multiplier (`w` x how far under
/// its break line, at least `shake_floor`).
pub(super) fn shaken(c: &crate::config::CombatCfg, w: f32, shake: f32) -> f32 {
    if shake <= 0.0 {
        return 1.0;
    }
    (1.0 - w * shake).max(c.shake_floor)
}

/// C5: the duel's stand-off cap: `human_max_beats` with the human's body in
/// it (the human ends a stand-off), else `max_beats`.
pub(super) fn beat_cap(c: &crate::config::CombatCfg, human: bool) -> u8 {
    if human {
        c.human_max_beats.max(c.max_beats).max(1)
    } else {
        c.max_beats.max(1)
    }
}
