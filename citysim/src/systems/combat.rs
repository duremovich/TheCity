//! The combat model C1, the beat (docs/COMBAT.md § 1-5, § 10 phase 1).
//!
//! A game abstraction between fictional agents of a simulated city: a fight
//! is a handful of **beats** resolved in one call ([`resolve_now`], the body
//! of `law::fight_inner`; every caller keeps its `(winner, loser, died)`,
//! the [`FightOutcome`] sits beside it on `World::last_fight`). No movement,
//! no joiners and no state across ticks: the live `Engagement` is C3.
//!
//! - **A beat.** Each standing side acts in reflex order: it may break
//!   (morale under its stance's line: it flees if its legs let it, else
//!   yields if the other side takes yields), else it attacks: the skill for
//!   its weapon's family (melee or shooting) against the target's evasion,
//!   the weapon kind's accuracy x its tier's quality, the stances. A hit
//!   lands on a region (`region_weights`) at a severity from the kind's
//!   damage table, less the armour on that region. A head hit may knock
//!   the body out; a Grave head or torso wound may kill (the kind's
//!   lethality x the caller's `kill_mult`; a subdue stance knocks out
//!   instead). Every wound takes blood at once; at none the body is down.
//! - **Wounds** ([`Wounds`]) stay on the body: an arm weakens the attack,
//!   a leg slows the dodge, the flight and the walk (`Kit.walk_mult`), the
//!   head makes a later knockout likelier and the work worse
//!   (`health::effects`). The hourly pass drains the blood of a bleeding
//!   wound into the M16b downed state (`wounded::down`), stops a Wound's
//!   bleeding on a roll and heals a wound after `heal_days` (C1's stub of
//!   C2's healing, the Clinic and first aid).
//! - **The skills** `melee`, `shooting`, `evasion` grow with hits landed
//!   and dodges; every gate reads [`power`].
//! - **No child is ever harmed**: a fight with a minor never starts
//!   (`law::resolve_fight_mods`), and every hit checks again.

use crate::components::{
    AssetLoc, Brain, Gang, Identity, Job, Kit, MemoryKind, Needs, Personality, Region, Severity, Skills, Wound, Wounds,
};
use crate::config::{WeaponFamily, WeaponKindCfg};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::personality::Drift;
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

const PURPOSE_BEAT: u64 = 0x434F_0001;
const PURPOSE_KIND: u64 = 0x434F_0002;
const PURPOSE_SHOOT: u64 = 0x434F_0003;
const PURPOSE_EVADE: u64 = 0x434F_0004;
const PURPOSE_BLEED: u64 = 0x434F_0005;

fn key(id: EntityId) -> u64 {
    u64::from(id.index) << 32 | u64::from(id.generation)
}

fn hash(world: &World, purpose: u64, a: u64, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, a, b)
}

// ---------------------------------------------------------------------------
// Stances, purposes, the outcome
// ---------------------------------------------------------------------------

/// A participant's stance (docs/COMBAT.md § 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    AllOut,
    Defensive,
    Subdue,
    HoldBack,
}

impl Stance {
    fn i(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Stance::AllOut => "all out",
            Stance::Defensive => "on guard",
            Stance::Subdue => "to subdue",
            Stance::HoldBack => "holding back",
        }
    }

    /// Does this side take the other's yield? All out does unless the
    /// purpose is lethal.
    fn takes_yield(self, lethal: bool) -> bool {
        !(self == Stance::AllOut && lethal)
    }
}

/// Why a fight was started: the caller's purpose (with `FightMods`'s
/// `arrest`, `kill_p` and `kill_mult`, it picks the stances).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Purpose {
    /// A brawl, a beating, an attack: stances from the personalities.
    #[default]
    Brawl,
    /// A gang raid or a turf fight: both sides all out.
    Raid,
    /// A visitor swinging at a door: the bouncer (`b`) subdues.
    Door,
}

/// How a participant's fight ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Won,
    Fled,
    Yielded,
    KnockedOut,
    Downed,
    Dead,
}

impl End {
    fn label(self) -> &'static str {
        match self {
            End::Won => "won",
            End::Fled => "fled",
            End::Yielded => "yielded",
            End::KnockedOut => "was knocked out",
            End::Downed => "went down bleeding",
            End::Dead => "was killed",
        }
    }
}

/// One side of a [`FightOutcome`].
#[derive(Clone, Debug, PartialEq)]
pub struct SideOut {
    pub id: EntityId,
    pub stance: Stance,
    /// The `[[weapon_kinds]]` row it fought with (0 the bare hand).
    pub kind: usize,
    pub end: End,
    /// The wounds it took, in order.
    pub took: Vec<(Region, Severity)>,
    pub landed: u8,
    pub dodged: u8,
}

/// The fight's result (docs/COMBAT.md § 1): the sides (`a` the starter
/// first), the winner, the beats run.
#[derive(Clone, Debug, PartialEq)]
pub struct FightOutcome {
    pub sides: [SideOut; 2],
    pub winner: EntityId,
    pub loser: EntityId,
    pub died: bool,
    pub beats: u8,
}

// ---------------------------------------------------------------------------
// Weapon kinds
// ---------------------------------------------------------------------------

/// The kinds table (row 0 the bare hand).
fn kinds(world: &World) -> &[WeaponKindCfg] {
    &world.config.weapon_kinds
}

fn kind_row(world: &World, k: usize) -> Option<&WeaponKindCfg> {
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

fn ranged(world: &World, k: usize) -> bool {
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

/// Seed `id`'s shooting and evasion when unset (-1): shooting from its
/// role's base x a keyed draw, evasion a keyed draw x the age curve.
pub fn seed_skills(world: &mut World, id: EntityId) {
    let Some(s) = world.comp::<Skills>(id) else { return };
    if s.shooting >= 0.0 && s.evasion >= 0.0 {
        return;
    }
    let c = &world.config.combat;
    let base = match world.comp::<Job>(id).map(|j| j.role) {
        Some(r) if c.shooting_posts.contains_key(&format!("{r:?}")) => c.shooting_posts[&format!("{r:?}")],
        _ if world.gang_of(id).is_some() => c.shooting_gang,
        _ => c.shooting_base,
    };
    let [lo, hi] = c.shooting_draw;
    let shoot = base * (lo + (hi - lo) * hash(world, PURPOSE_SHOOT, key(id), 0));
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

/// Every body with skills unset (`World::new`, and at midnight for the
/// day's arrivals and those come of age).
pub fn seed_all(world: &mut World) {
    let ids: Vec<EntityId> = world.with::<Skills>();
    for id in ids {
        seed_skills(world, id);
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
fn carried_note(world: &World, id: EntityId) -> Option<&'static str> {
    let w = world.comp::<Wounds>(id)?;
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

struct Fighter {
    id: EntityId,
    stance: Stance,
    kind: usize,
    quality: f32,
    /// The skill its weapon uses, less frailty, plus the crew bonus.
    attack: f32,
    evasion: f32,
    strength: f32,
    reflex: f32,
    morale: f32,
    skin: f32,
    vest: bool,
    ready_at: u8,
    attacked: bool,
    cornered: bool,
    robot: bool,
    end: Option<End>,
    took: Vec<(Region, Severity)>,
    landed: u8,
    dodged: u8,
    melee_hits: u8,
    shots: u8,
}

/// The stance `id` takes as the starter (`attacker`) or the one set upon.
fn stance_for(world: &World, id: EntityId, attacker: bool, mods: &crate::systems::law::FightMods) -> Stance {
    if crate::systems::robots::is_robot(world, id) {
        return Stance::AllOut;
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

fn fighter(world: &World, id: EntityId, stance: Stance, attacker: bool, bonus: f32) -> Fighter {
    let c = &world.config.combat;
    let robot = crate::systems::robots::is_robot(world, id);
    let mut kind = kind_of(world, id);
    let tier = crate::systems::weapons::tier(world, id);
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
    let attack = (skill * fx.attack_mult - fx.frail_fight) * jm + bonus;
    let evasion = (evasion + c.reflex_evade * reflex) * fx.evade_mult * jm;
    let courage = crate::systems::law::courage(world, id);
    let morale = if robot {
        f32::INFINITY
    } else {
        c.morale_base + c.courage_w * (courage - 0.5) + c.stance_morale[stance.i()] + bonus
    };
    let draw = if attacker { 0 } else { kind_row(world, kind).map_or(0, |r| r.draw) };
    Fighter {
        id,
        stance,
        kind,
        quality,
        attack,
        evasion,
        strength: kit.map_or(0.0, |k| k.strength),
        reflex,
        morale,
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
    }
}

/// The roll stream of one fight: a world-stream salt, hashed per draw.
struct Dice {
    salt: u64,
    n: u64,
}

impl Dice {
    fn roll(&mut self, world: &World) -> f32 {
        self.n += 1;
        hash(world, PURPOSE_BEAT, self.salt, self.n)
    }
}

fn pick_weighted(r: f32, w: &[f32]) -> usize {
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
/// § 1, off camera and on: C3 makes it live for Full bodies). Writes the
/// wounds, the memories, the skills, the deaths and the downed; returns the
/// outcome (the callers' `(winner, loser, died)` is read off it).
pub fn resolve_now(world: &mut World, a: EntityId, b: EntityId, mods: crate::systems::law::FightMods) -> FightOutcome {
    use rand::Rng;
    for x in [a, b] {
        seed_skills(world, x);
    }
    let lethal = mods.kill_p.is_some();
    let (sa, sb) = (stance_for(world, a, true, &mods), stance_for(world, b, false, &mods));
    let notes = [carried_note(world, a), carried_note(world, b)];
    let mut f = [fighter(world, a, sa, true, mods.a_bonus), fighter(world, b, sb, false, 0.0)];
    let mut dice = Dice { salt: world.rng.world().random::<u64>(), n: 0 };
    let order = if f[1].reflex > f[0].reflex { [1, 0] } else { [0, 1] };
    let max_beats = world.config.combat.max_beats.max(1);
    let mut beats = 0u8;
    'fight: for beat in 0..max_beats {
        beats = beat + 1;
        for &i in &order {
            let j = 1 - i;
            if f[0].end.is_some() || f[1].end.is_some() {
                break 'fight;
            }
            let c = &world.config.combat;
            // The break: flee, else yield, else cornered (fights on).
            if !f[i].cornered && f[i].morale < c.break_at + c.break_shift[f[i].stance.i()] {
                if let Some(e) = break_off(world, &f, i, lethal, &mut dice) {
                    f[i].end = Some(e);
                    break 'fight;
                }
                f[i].cornered = true;
            }
            if f[i].stance == Stance::HoldBack && !f[i].attacked {
                continue;
            }
            if beat < f[i].ready_at {
                continue;
            }
            strike(world, &mut f, i, j, &mods, a, b, &mut dice);
        }
    }
    if f[0].end.is_none() && f[1].end.is_none() {
        // Out of beats: the lower morale gives way (flees if it can).
        let i = if f[0].morale < f[1].morale { 0 } else { 1 };
        let e = break_off(world, &f, i, lethal, &mut dice).unwrap_or(End::Yielded);
        f[i].end = Some(e);
    }
    let li = if f[0].end.is_some() { 0 } else { 1 };
    let (winner, loser) = (f[1 - li].id, f[li].id);
    f[1 - li].end = Some(End::Won);
    // A lethal purpose's win (a Hunt's strike) finishes the beaten body.
    if lethal && winner == a && f[li].end != Some(End::Dead) && !f[li].robot {
        let p = mods.kill_p.unwrap_or(0.0) * mods.kill_mult;
        if dice.roll(world) < p {
            f[li].end = Some(End::Dead);
        }
    }
    let out = finish(world, f, winner, loser, beats, &mods, a, b, notes);
    world.last_fight = Some(out.clone());
    out
}

/// The broken side `i` gets away: flees on its legs, else yields when the
/// other side takes yields; `None`: cornered.
fn break_off(world: &World, f: &[Fighter; 2], i: usize, lethal: bool, dice: &mut Dice) -> Option<End> {
    let c = &world.config.combat;
    let fx = crate::systems::health::effects(world, f[i].id);
    let p = (c.flee_p + c.flee_evade_w * f[i].evasion) * fx.evade_mult;
    let lame = world.comp::<Wounds>(f[i].id).is_some_and(|w| {
        w.list.iter().any(|x| x.region.is_leg() && x.severity == Severity::Grave)
            || w.list.iter().filter(|x| x.region.is_leg() && x.severity >= Severity::Wound).count() >= 2
    });
    if !lame && dice.roll(world) < p {
        return Some(End::Fled);
    }
    // Only the starter's purpose is lethal.
    let other = &f[1 - i];
    let lethal_here = lethal && i == 1;
    other.stance.takes_yield(lethal_here).then_some(End::Yielded)
}

#[allow(clippy::too_many_arguments)]
fn strike(
    world: &mut World,
    f: &mut [Fighter; 2],
    i: usize,
    j: usize,
    mods: &crate::systems::law::FightMods,
    a: EntityId,
    b: EntityId,
    dice: &mut Dice,
) {
    f[j].attacked = true;
    // The under-18 rule, at every hit (CLAUDE.md).
    if crate::systems::ages::is_protected(world, f[j].id) {
        f[i].end = Some(End::Yielded);
        return;
    }
    let c = world.config.combat.clone();
    let row = kind_row(world, f[i].kind).cloned().unwrap_or_default();
    let p = (c.hit_base + c.attack_w * f[i].attack + row.accuracy * f[i].quality + c.stance_attack[f[i].stance.i()]
        - c.evade_w * (f[j].evasion + c.stance_evade[f[j].stance.i()]))
    .clamp(c.hit_min, c.hit_max);
    if dice.roll(world) >= p {
        f[j].dodged += 1;
        return;
    }
    f[i].landed += 1;
    if row.family == WeaponFamily::Ranged {
        f[i].shots += 1;
    } else {
        f[i].melee_hits += 1;
    }
    f[i].morale += c.morale_landed;
    if f[j].robot {
        // A robot takes no wounds (robot fights go `law::resolve_robot_fight`).
        f[j].morale -= c.morale_hit[1];
        return;
    }
    let region = Region::ALL[pick_weighted(dice.roll(world), &c.region_weights)];
    let mut w = row.damage;
    let q = f[i].quality.max(0.01);
    w[0] /= q;
    w[2] *= q;
    if row.family == WeaponFamily::Melee {
        w[2] *= 1.0 + c.strength_grave * f[i].strength;
    }
    let mut sev = [Severity::Graze, Severity::Wound, Severity::Grave][pick_weighted(dice.roll(world), &w)];
    // Armour on the region takes it down a severity (a graze stopped).
    let armour = if region == Region::Torso && f[j].vest { f[j].skin.max(c.vest_torso) } else { f[j].skin };
    if armour > 0.0 && dice.roll(world) < armour * c.armour_w {
        match sev.down() {
            Some(s) => sev = s,
            None => return,
        }
    }
    let subdue = f[i].stance == Stance::Subdue;
    let mut ko = false;
    if subdue && sev == Severity::Grave {
        // A knockout replaces a Grave wound where it can.
        sev = Severity::Wound;
        ko = true;
    }
    let mut dead = false;
    if sev == Severity::Grave && matches!(region, Region::Head | Region::Torso) && !subdue {
        let p = row.lethality * q * mods.kill_mult;
        dead = dice.roll(world) < p;
    }
    if !dead && !ko && region == Region::Head {
        let fx = crate::systems::health::effects(world, f[j].id);
        let m = if row.blunt { c.blunt_ko_mult } else { 1.0 };
        ko = dice.roll(world) < c.ko_p[sev.index()] * m + fx.ko_add;
    }
    f[j].took.push((region, sev));
    f[j].morale -= c.morale_hit[sev.index()];
    if dead {
        f[j].end = Some(End::Dead);
        return;
    }
    // A bleed-out from it is a Murder when the starter dealt it to the one
    // he set upon and is no guard.
    let murder = f[i].id == a && f[j].id == b && !crate::systems::law::is_guard(world, a);
    let blood = add_wound(world, f[j].id, region, sev, Some(f[i].id), murder, row.blunt);
    // The wound's effects reach the next beat.
    let base = fighter(world, f[j].id, f[j].stance, false, 0.0);
    f[j].attack = base.attack + if f[j].id == a { mods.a_bonus } else { 0.0 };
    f[j].evasion = base.evasion;
    if blood <= 0.0 {
        f[j].end = Some(End::Downed);
    } else if ko {
        f[j].end = Some(End::KnockedOut);
    }
}

/// Put a wound on `id` (bleeding by the table: the torso from a Wound up,
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
    let c = &world.config.combat;
    let bleeding = !blunt && (severity == Severity::Grave || (region == Region::Torso && severity == Severity::Wound));
    let loss = if blunt { 0.0 } else { c.blood_hit[severity.index()] };
    let w = Wound { region, severity, bleeding, since: world.tick, treated: false, by, murder };
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
fn finish(
    world: &mut World,
    f: [Fighter; 2],
    winner: EntityId,
    loser: EntityId,
    beats: u8,
    mods: &crate::systems::law::FightMods,
    a: EntityId,
    b: EntityId,
    notes: [Option<&'static str>; 2],
) -> FightOutcome {
    let li = if f[0].id == loser { 0 } else { 1 };
    let loser_end = f[li].end.unwrap_or(End::Yielded);
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
    // Growth: hits landed and dodges (C1, replacing the winner's flat +0.01).
    let c = world.config.combat.clone();
    for x in &f {
        if let Some(s) = world.comp_mut::<Skills>(x.id) {
            s.melee = (s.melee + c.melee_gain * f32::from(x.melee_hits)).min(1.0);
            s.shooting = (s.shooting.max(0.0) + c.shooting_gain * f32::from(x.shots)).min(1.0);
            s.evasion = (s.evasion.max(0.0) + c.evasion_gain * f32::from(x.dodged)).min(1.0);
        }
    }
    crate::systems::social::fought(world, winner, loser);
    let mut died = loser_end == End::Dead;
    let mut end = loser_end;
    if loser_end == End::Downed {
        let murder = loser == b && !crate::systems::law::is_guard(world, a);
        if !go_down(world, loser, Some(winner), murder) {
            died = true;
            end = End::Dead;
        }
    }
    if died {
        world.kill_by(loser, crate::components::DeathCause::Violence, Some(winner));
    }
    let arms = crate::systems::weapons::fight_tiers(world, a, b);
    crate::systems::weapons::after_fight(world, winner, loser, died, arms);
    crate::systems::fashion::after_fight(world, winner, loser);
    crate::systems::braindance::after_fight(world, winner);
    let side = |x: &Fighter, e: End| SideOut {
        id: x.id,
        stance: x.stance,
        kind: x.kind,
        end: e,
        took: x.took.clone(),
        landed: x.landed,
        dodged: x.dodged,
    };
    let ends = if li == 0 { [end, End::Won] } else { [End::Won, end] };
    let out = FightOutcome { sides: [side(&f[0], ends[0]), side(&f[1], ends[1])], winner, loser, died, beats };
    let text = line(world, &out, notes);
    world.push_event(EventKind::Fight, &[a, b], text);
    out
}

/// Down by blood loss (the M16b state); at `wounded_quota` (or with
/// `[wounded]` off) the body dies instead (`wounds_capped`). True: down.
fn go_down(world: &mut World, id: EntityId, by: Option<EntityId>, murder: bool) -> bool {
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
fn line(world: &World, out: &FightOutcome, notes: [Option<&'static str>; 2]) -> String {
    let who = |s: &SideOut, note: Option<&str>| {
        let w = kind_row(world, s.kind).filter(|_| s.kind > 0).map_or("bare hands".to_string(), |r| r.id.clone());
        let n = note.map_or(String::new(), |n| format!(", {n}"));
        format!("{} ({}, {w}{n})", world.name_of(s.id), s.stance.label())
    };
    let mut text =
        format!("{} fought {}, {} beats", who(&out.sides[0], notes[0]), who(&out.sides[1], notes[1]), out.beats);
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
        let hurts: Vec<String> = seen
            .iter()
            .map(|&((r, v), n)| {
                if n > 1 {
                    format!("{} {} x{n}", r.label(), v.label())
                } else {
                    format!("{} {}", r.label(), v.label())
                }
            })
            .collect();
        text.push_str(&format!("; {} hit: {}", world.name_of(s.id), hurts.join(", ")));
    }
    let l = if out.sides[0].id == out.loser { &out.sides[0] } else { &out.sides[1] };
    text.push_str(&format!("; {} {}", world.name_of(l.id), l.end.label()));
    text
}

// ---------------------------------------------------------------------------
// The hourly pass: bleeding and healing
// ---------------------------------------------------------------------------

/// Hourly (and seeding at midnight): every hurt body's bleeding drains its
/// blood (a Wound may stop on a roll; a Grave bleeds until a Clinic
/// stabilises it, the M16b downed state's Stabilise); at no blood it goes
/// down; a body not bleeding gets blood back; a wound heals `heal_days`
/// after it was dealt (C1's stub of C2's healing).
pub fn hourly(world: &mut World) {
    if crate::systems::midnight::due(world, crate::systems::midnight::Mid::Tail) {
        seed_all(world);
    }
    if !world.tick.is_multiple_of(TICKS_PER_HOUR) || world.hurt.is_empty() {
        return;
    }
    let now = world.tick;
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
        match down {
            // Stabilised at a Clinic: the bleeding stops (treated).
            Some(true) => {
                for x in w.list.iter_mut().filter(|x| x.bleeding) {
                    x.bleeding = false;
                    x.treated = true;
                }
                w.blood = w.blood.max(0.3);
            }
            // Down and bleeding: the M16b timer has it.
            Some(false) => {}
            None => {
                for (k, x) in w.list.iter_mut().enumerate() {
                    if x.bleeding && x.severity < Severity::Grave {
                        let r = hash(world, PURPOSE_BLEED, key(id), now ^ (k as u64) << 48);
                        if x.severity == Severity::Graze || r < c.stop_p_wound {
                            x.bleeding = false;
                            lines.push(format!(
                                "the bleeding from {}'s {} stopped",
                                world.name_of(id),
                                x.region.label()
                            ));
                        }
                    }
                }
                let drain: f32 = w.list.iter().filter(|x| x.bleeding).map(|x| c.bleed_hour[x.severity.index()]).sum();
                if drain > 0.0 {
                    w.blood = (w.blood - drain).max(0.0);
                } else {
                    w.blood = (w.blood + c.regen_hour).min(1.0);
                }
            }
        }
        // Healing (the stub): a wound not bleeding is gone after its days.
        let before = w.list.len();
        let mut healed: Vec<String> = Vec::new();
        w.list.retain(|x| {
            let gone = !x.bleeding && now >= x.since + Tick::from(c.heal_days[x.severity.index()]) * TICKS_PER_DAY;
            if gone && x.severity >= Severity::Wound {
                healed.push(x.region.label().to_string());
            }
            !gone
        });
        for r in healed {
            lines.push(format!("{}'s {r} has healed", world.name_of(id)));
        }
        let blood = w.blood;
        let cause = w.list.iter().filter(|x| x.bleeding).max_by_key(|x| x.severity).map(|x| (x.by, x.murder));
        let legs_changed = before != w.list.len();
        if w.list.is_empty() && w.blood >= 1.0 {
            world.remove::<Wounds>(id);
            world.hurt.remove(&id);
        } else if let Some(x) = world.comp_mut::<Wounds>(id) {
            *x = w;
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
        }
    }
}
