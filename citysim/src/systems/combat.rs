//! The combat model C1, the beat, and C2, wounds that last (docs/COMBAT.md
//! § 1-5, § 10 phases 1-2).
//!
//! A game abstraction between fictional agents of a simulated city: a fight
//! is a handful of **beats** between two duellists ([`resolve_now`] runs one
//! duel in one call; the [`FightOutcome`] sits on `World::last_fight`).
//! - **C3a, the engagement** ([`Engagement`], [`engage`]): two sides of
//!   members, a duel at a time (the fronts; a loser leaves his side and the
//!   next steps up). Every party Full: it lives in `World::combat`
//!   across ticks ([`step`], `beats_per_tick` a tick after exec; its bodies
//!   neither think nor act), else it runs to its end in the call. Each
//!   caller's tail is a [`Tail`], run once either way; a caller whose step
//!   needs the answer holds it `Running` ([`take_done`]). A body that breaks
//!   and flees runs (a flee plan); one who moves out of reach has fled.
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
//!   wound into the M16b downed state (`wounded::down`) and stops a Wound's
//!   bleeding on a roll.
//! - **C2, wounds that last** (§ 2): a wound heals at its `heal_at`, sooner
//!   when seen to (the Treat goal's patch-up, `customers::hurt`, priced by
//!   the worst wound; the M16b Stabilise); first aid from a friend, kin or a
//!   gang-mate beside the body; a manual post turns a bad arm or leg away
//!   ([`unfit`]: the shift and its pay lost); a healed Grave may leave a
//!   maim, a lost limb (also severed at the hit) until a ripperdoc fits
//!   chrome ([`fitted`]), and a scar (`Wounds.marks`); the off-screen
//!   beatings write wounds ([`stat_wounds`]). Age slows the fighter and
//!   breaks the bone; a post trains its skills at midnight; the pit's bout
//!   is a `Bout` here.
//! - **The skills** `melee`, `shooting`, `evasion` grow with hits landed
//!   and dodges; every gate reads [`power`].
//! - **No child is ever harmed**: a fight with a minor never starts
//!   (`law::resolve_fight_mods`), and every hit checks again.

use crate::components::{
    AssetLoc, Brain, Gang, Identity, Job, Kit, Mark, MemoryKind, Needs, Personality, Position, Region, Severity,
    Skills, Slot, Wound, Wounds,
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
const PURPOSE_HEAL: u64 = 0x434F_0006;
const PURPOSE_STAT: u64 = 0x434F_0007;

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
    /// C2, C1's wrong-read (c): a FightPit's bout. Both subdue, bare hands;
    /// sanctioned: no memories of a fight, no edges, no weapons' tail (the
    /// pit writes its own `Bout` memories).
    Bout,
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
    /// C3a: the ticks a live duel ran past its first (0: in one tick).
    pub ticks: u32,
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
fn carried_note(world: &World, id: EntityId) -> Option<&'static str> {
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
    if mods.why == Purpose::Bout {
        return Stance::Subdue;
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
fn over_age(world: &World, id: EntityId) -> f32 {
    world.comp::<Identity>(id).map_or(0.0, |i| {
        (i.age_days as f32 / crate::time::DAYS_PER_YEAR as f32 - world.config.combat.age_from).max(0.0)
    })
}

fn fighter(world: &World, id: EntityId, stance: Stance, attacker: bool, bonus: f32, bout: bool) -> Fighter {
    let c = &world.config.combat;
    let robot = crate::systems::robots::is_robot(world, id);
    let mut kind = if bout { 0 } else { kind_of(world, id) };
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
#[derive(Clone, Debug)]
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
struct Duel {
    f: [Fighter; 2],
    dice: Dice,
    order: [usize; 2],
    beat: u8,
    notes: [Option<&'static str>; 2],
    /// The tick it began (a live duel's span).
    since: Tick,
    /// Where the two stood at the start (a live duel: one moved has left)
    /// and whether they were within `reach_tiles` of each other then.
    at: [(crate::components::TilePos, Option<EntityId>); 2],
    near: bool,
    names: [String; 2],
}

fn duel_start(world: &mut World, a: EntityId, b: EntityId, mods: &crate::systems::law::FightMods) -> Duel {
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
    }
}

/// Run beats of `d`, each taken off `left`, until it ends (true) or `left`
/// runs out (false). `log`: a live engagement's ring log.
fn duel_beats(
    world: &mut World,
    d: &mut Duel,
    mods: &crate::systems::law::FightMods,
    left: &mut u32,
    mut log: Option<&mut std::collections::VecDeque<BeatLog>>,
) -> bool {
    let lethal = mods.kill_p.is_some();
    let (a, b) = (d.f[0].id, d.f[1].id);
    let max_beats = world.config.combat.max_beats.max(1);
    let cap = world.config.combat.log_beats;
    let note = |log: &mut Option<&mut std::collections::VecDeque<BeatLog>>, tick, by, on, blow| {
        if let Some(l) = log.as_deref_mut() {
            if l.len() >= cap.max(1) {
                l.pop_front();
            }
            l.push_back(BeatLog { tick, by, on, blow });
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
            note(&mut log, world.tick, d.f[i].id, d.f[1 - i].id, Blow::Broke(e));
            return true;
        }
        if *left == 0 {
            return false;
        }
        *left -= 1;
        let beat = d.beat;
        d.beat += 1;
        let order = d.order;
        for i in order {
            let j = 1 - i;
            if d.f[0].end.is_some() || d.f[1].end.is_some() {
                break;
            }
            let c = &world.config.combat;
            // The break: flee, else yield, else cornered (fights on).
            if !d.f[i].cornered && d.f[i].morale < c.break_at + c.break_shift[d.f[i].stance.i()] {
                if let Some(e) = break_off(world, &d.f, i, lethal, &mut d.dice) {
                    d.f[i].end = Some(e);
                    note(&mut log, world.tick, d.f[i].id, d.f[j].id, Blow::Broke(e));
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
            let took = d.f[j].took.len();
            strike(world, &mut d.f, i, j, mods, a, b, &mut d.dice);
            let blow = if d.f[j].took.len() > took { d.f[j].took.last().map(|&(r, v)| Blow::Hit(r, v)) } else { None };
            note(&mut log, world.tick, d.f[i].id, d.f[j].id, blow.unwrap_or(Blow::Miss));
            if let Some(e) = d.f[j].end {
                note(&mut log, world.tick, d.f[j].id, d.f[i].id, Blow::Broke(e));
            }
        }
    }
}

/// The ended duel's outcome: the winner, a lethal purpose's finish, the
/// tail of the fight (`finish`).
fn duel_end(world: &mut World, mut d: Duel, mods: &crate::systems::law::FightMods) -> FightOutcome {
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
    let out = finish(world, d.f, winner, loser, (d.beat, ticks), mods, a, b, d.notes);
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
    // C2 (a): an old body breaks easier.
    w[2] *= 1.0 + c.age_grave * over_age(world, f[j].id);
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
    // C2: a Grave limb hit from a blade or a gun may take the limb.
    if sev == Severity::Grave && (region.is_arm() || region.is_leg()) && !row.blunt && dice.roll(world) < c.sever_p {
        let how = if row.family == WeaponFamily::Ranged { "shot to ruin" } else { "hacked off" };
        lose_limb(world, f[j].id, region, &format!("{how} by {} with a {}", world.name_of(f[i].id), row.id));
    }
    // The wound's effects reach the next beat.
    let base = fighter(world, f[j].id, f[j].stance, false, 0.0, mods.why == Purpose::Bout);
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
fn finish(
    world: &mut World,
    f: [Fighter; 2],
    winner: EntityId,
    loser: EntityId,
    (beats, ticks): (u8, u32),
    mods: &crate::systems::law::FightMods,
    a: EntityId,
    b: EntityId,
    notes: [Option<&'static str>; 2],
) -> FightOutcome {
    let li = if f[0].id == loser { 0 } else { 1 };
    let loser_end = f[li].end.unwrap_or(End::Yielded);
    let bout = mods.why == Purpose::Bout;
    if !bout {
        sequel(world, a, b, winner, loser, mods);
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
    if died {
        world.kill_by(loser, crate::components::DeathCause::Violence, Some(winner));
    }
    if !bout {
        let arms = crate::systems::weapons::fight_tiers(world, a, b);
        crate::systems::weapons::after_fight(world, winner, loser, died, arms);
        crate::systems::fashion::after_fight(world, winner, loser);
        crate::systems::braindance::after_fight(world, winner);
    }
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
    let out = FightOutcome { sides: [side(&f[0], ends[0]), side(&f[1], ends[1])], winner, loser, died, beats, ticks };
    let text = line(world, &out, notes);
    world.push_event(EventKind::Fight, &[a, b], text);
    out
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
    // C3a: a live duel that ran across ticks says how long.
    if out.ticks > 0 {
        text.push_str(&format!(" over {} min", out.ticks + 1));
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
// C2: wounds that last (docs/COMBAT.md § 2)
// ---------------------------------------------------------------------------

/// The tick `x` heals (an old save's wound: `since` + its days).
fn heal_at(x: &Wound, c: &crate::config::CombatCfg) -> Tick {
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
pub fn urgency(world: &World, id: EntityId) -> f32 {
    worst_untreated(world, id).map_or(0.0, |s| world.config.combat.treat_urgency[s.index()])
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
        if (r.is_arm() || r.is_leg()) && !gravek.blunt && hash(world, PURPOSE_STAT, key(id), salt ^ 0xA2) < c.sever_p {
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

// ---------------------------------------------------------------------------
// C3a: the live engagement (docs/COMBAT.md § 1)
// ---------------------------------------------------------------------------

/// What a beat did, for the ring log (the view's replay in C4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blow {
    Miss,
    Hit(Region, Severity),
    /// `by` broke off, was knocked out, went down or died.
    Broke(End),
}

/// One entry of an engagement's ring log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeatLog {
    pub tick: Tick,
    pub by: EntityId,
    pub on: EntityId,
    pub blow: Blow,
}

/// One duel's result as a caller reads it: C1's `(winner, loser, died)`,
/// the two (`a` of side 0, `b` of side 1), the loser's end, their names
/// taken before it (a beaten robot is wrecked in it). `fought` false: no
/// blow was struck (a child, a party gone): the starter stands, as C1's
/// backstop read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Clash {
    pub a: EntityId,
    pub b: EntityId,
    pub winner: EntityId,
    pub loser: EntityId,
    pub died: bool,
    pub end: End,
    pub fought: bool,
    pub names: [String; 2],
}

/// A caller's action held while its engagement runs (`Running`); the tail
/// leaves the verdict for the action's next step ([`take_done`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Attack,
    AttackGuard,
    Abduct,
    GuardBody,
}

/// The purpose's on-end hook: each caller's tail, run once whether the
/// fight was live or resolved in one call (C3a: one tail per caller).
#[derive(Clone, Debug)]
pub enum Tail {
    /// None: the caller reads the triple (`law::resolve_fight_mods`).
    Bare,
    /// The Attack action (the attacker is side 0).
    Attack {
        victim: EntityId,
    },
    /// M16a: a Guard taker on post meets the attack first.
    AttackGuard {
        victim: EntityId,
        guard: EntityId,
    },
    /// M13 D36: the abduction (the abductor's Abduct step).
    Abduct {
        victim: EntityId,
    },
    /// W35: a guard over a corpse fights the stripper (the Strip or Rip step).
    GuardBody {
        stripper: EntityId,
    },
    /// The cuffs fought (the guard side 0, the backups after him).
    Arrest {
        berserk: bool,
    },
    /// M16b: a blackmailed culprit goes for the witness.
    Silence(Box<crate::systems::consequence::Silence>),
    /// A fight in the cells.
    Cells(Box<crate::systems::consequence::CellFight>),
    /// The crew's reckoning.
    Reckoning(Box<crate::systems::crew::Reckon>),
    /// A brave squatter stands against the gang's claim (the squatter side 0).
    Squat {
        gname: String,
    },
    /// The pressure pass: a miserable body's brawl.
    Brawl(Box<crate::systems::misery::BrawlTail>),
    /// Protection: the owner beaten.
    Beating(Box<crate::systems::protection::Beating>),
    /// A patrol's attack on a found adult for their chrome.
    Scan {
        tile: crate::components::TilePos,
    },
    /// Underwater: the collector's beating, the stiffed worker's swing.
    Dun(Box<crate::systems::underwater::Dun>),
    Confront(Box<crate::systems::underwater::Confront>),
    /// `raid::fight_out`'s pairings (each an Assault or a Murder by side 0).
    Pairs {
        door: crate::components::TilePos,
        place: String,
    },
    /// A gang's raid on a rival Hideout: the pairings, then the raid's end.
    Raid(Box<crate::systems::raid::RaidEnd>),
}

/// A fight across ticks (C3a): two sides of members in order (the front
/// fights; a loser leaves its side and the next steps up), the caller's
/// mods and tail, the duels' results, the ring log. Not saved: a save
/// resolves every engagement first ([`settle_all`]).
#[derive(Clone, Debug)]
pub struct Engagement {
    pub sides: [Vec<EntityId>; 2],
    pub first: [Option<EntityId>; 2],
    pub mods: crate::systems::law::FightMods,
    pub tail: Tail,
    pub start: Tick,
    pub live: bool,
    /// Side 0 breaks at this many losses (a corp raid's crew).
    pub max_losses: usize,
    pub losses: [usize; 2],
    pub deaths: usize,
    /// Who lost a duel and lived, by side.
    pub beaten: [Vec<EntityId>; 2],
    pub clashes: Vec<Clash>,
    pub beats: u32,
    pub log: std::collections::VecDeque<BeatLog>,
    duel: Option<Duel>,
}

impl Engagement {
    /// The last fought duel's result, or C1's "no blow" (the starter stands).
    pub fn result(&self) -> Clash {
        if let Some(c) = self.clashes.iter().rev().find(|c| c.fought).or(self.clashes.last()) {
            return c.clone();
        }
        let a = self.first[0].or(self.first[1]).expect("an engagement has a party");
        let b = self.first[1].unwrap_or(a);
        Clash { a, b, winner: a, loser: b, died: false, end: End::Yielded, fought: false, names: Default::default() }
    }

    /// Everyone still on a side.
    pub fn members(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.sides[0].iter().chain(self.sides[1].iter()).copied()
    }
}

/// What [`engage`] did: the fight is live (the caller's action holds), or
/// it was resolved in the call and its tail ran (the tail's verdict).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Begun {
    Live,
    Over(bool),
}

/// C3a's state on the world (`World::combat`, boxed: the world's serde
/// visitor sits near a debug test thread's stack): the live engagements by
/// id, the bodies in them, the held steps' verdicts, the fled bodies whose
/// flee plans wait for `step`, the last gang raid's outcome. Not saved (a
/// save resolves the engagements first, [`settle_all`]).
#[derive(Clone, Debug, Default)]
pub struct Live {
    pub engagements: std::collections::BTreeMap<u32, Engagement>,
    pub next: u32,
    /// A body in a live engagement -> its id.
    pub engaged: std::collections::BTreeMap<EntityId, u32>,
    /// A held step's engagement ended: (tick, which, the verdict).
    pub done: std::collections::BTreeMap<EntityId, (Tick, Held, bool)>,
    pub fleeing: Vec<EntityId>,
    pub last_raid: Option<crate::systems::raid::Outcome>,
}

/// Is `id` in a live engagement (it neither thinks nor acts)?
pub fn engaged(world: &World, id: EntityId) -> bool {
    !world.combat.engaged.is_empty() && world.combat.engaged.contains_key(&id)
}

/// A caller's 1-on-1 fight: live when both are Full, else at once.
pub fn fight(world: &mut World, a: EntityId, b: EntityId, mods: crate::systems::law::FightMods, tail: Tail) -> Begun {
    engage(world, [vec![a], vec![b]], mods, tail, true, usize::MAX)
}

/// The fight resolved in one call with no tail: the triple's source.
pub fn clash_now(world: &mut World, a: EntityId, b: EntityId, mods: crate::systems::law::FightMods) -> Clash {
    run_now(world, [vec![a], vec![b]], mods, Tail::Bare, usize::MAX).result()
}

/// Sides fought out in one call (the tail runs); the ended engagement.
pub fn run_now(
    world: &mut World,
    sides: [Vec<EntityId>; 2],
    mods: crate::systems::law::FightMods,
    tail: Tail,
    max_losses: usize,
) -> Engagement {
    let mut e = build(world, sides, mods, tail, max_losses);
    advance(world, &mut e, u32::MAX);
    end(world, &mut e);
    e
}

fn build(
    world: &mut World,
    mut sides: [Vec<EntityId>; 2],
    mods: crate::systems::law::FightMods,
    tail: Tail,
    max_losses: usize,
) -> Engagement {
    let first = [sides[0].first().copied(), sides[1].first().copied()];
    // No child is ever a party (CLAUDE.md): counted as C1's backstop was.
    for s in &mut sides {
        let n = s.len();
        s.retain(|&x| !crate::systems::ages::is_protected(world, x));
        if s.len() < n && n == 1 {
            crate::systems::ownership::jobs2_row(world).harm_blocked += 1;
        }
    }
    Engagement {
        sides,
        first,
        mods,
        tail,
        start: world.tick,
        live: false,
        max_losses,
        losses: [0; 2],
        deaths: 0,
        beaten: [Vec::new(), Vec::new()],
        clashes: Vec::new(),
        beats: 0,
        log: std::collections::VecDeque::new(),
        duel: None,
    }
}

/// Start a fight between `sides` (C3a § 1). Live when `live_ok` and every
/// party is Full and free (in camera range a party not Full is promoted
/// first); else it runs to its end here, tail and all.
pub fn engage(
    world: &mut World,
    sides: [Vec<EntityId>; 2],
    mods: crate::systems::law::FightMods,
    tail: Tail,
    live_ok: bool,
    max_losses: usize,
) -> Begun {
    let mut e = build(world, sides, mods, tail, max_losses);
    if live_ok && go_live(world, &e) {
        e.live = true;
        let id = world.combat.next;
        world.combat.next = world.combat.next.wrapping_add(1);
        for x in e.members().collect::<Vec<_>>() {
            world.combat.engaged.insert(x, id);
        }
        world.combat.engagements.insert(id, e);
        return Begun::Live;
    }
    advance(world, &mut e, u32::MAX);
    Begun::Over(end(world, &mut e))
}

/// Every party a Full body, none in another fight; one the camera holds
/// (or the human drives) brings the rest up to Full first (`OPEN-149`).
fn go_live(world: &mut World, e: &Engagement) -> bool {
    use crate::components::Lod;
    let ids: Vec<EntityId> = e.members().collect();
    if ids.is_empty() || e.sides.iter().any(|s| s.is_empty()) {
        return false;
    }
    let lod = |w: &World, x: EntityId| w.comp::<Brain>(x).map(|b| b.lod);
    if ids.iter().any(|&x| lod(world, x).is_none() || engaged(world, x)) {
        return false;
    }
    let seen = ids.iter().any(|&x| crate::systems::player::drives(world, x) || world.ring.holds(x));
    if seen {
        let place = ids.iter().find_map(|&x| world.comp::<Position>(x).and_then(|p| p.building));
        for &x in &ids {
            if lod(world, x) != Some(Lod::Full) && !world.has::<crate::components::Sentence>(x) {
                crate::systems::lod::set_lod_at(world, x, Lod::Full, place);
            }
        }
    }
    ids.iter().all(|&x| lod(world, x) == Some(Lod::Full))
}

/// The duel's instant cases (C1's `fight_inner`): a robot on a side (one
/// roll, `law::resolve_robot_fight`), a downed party finished where it lies.
fn instant(world: &mut World, a: EntityId, b: EntityId, mods: &crate::systems::law::FightMods) -> Option<Clash> {
    if crate::systems::robots::is_robot(world, a) || crate::systems::robots::is_robot(world, b) {
        let names = [world.name_of(a), world.name_of(b)];
        let (winner, loser, died) = crate::systems::law::resolve_robot_fight(world, a, b, *mods);
        let end = if died { End::Dead } else { End::Yielded };
        return Some(Clash { a, b, winner, loser, died, end, fought: true, names });
    }
    if !world.wounded.is_empty() {
        let (down_a, down_b) = (world.wounded.contains_key(&a), world.wounded.contains_key(&b));
        if down_a != down_b {
            let names = [world.name_of(a), world.name_of(b)];
            let (winner, loser) = if down_b { (a, b) } else { (b, a) };
            crate::systems::wounded::finish(world, loser, winner);
            return Some(Clash { a, b, winner, loser, died: true, end: End::Dead, fought: true, names });
        }
    }
    None
}

/// Every fought duel's after-tail (C1's `resolve_fight_mods`): a beaten
/// member on his own blocks provokes the holder; a body jacked in is
/// dumped from its run.
fn after_duel(world: &mut World, c: &Clash) {
    if world.config.turf.as_ref().is_some_and(|t| t.war.is_some()) {
        crate::systems::turf::beaten(world, c.winner, c.loser);
    }
    if !world.runner_of.is_empty() {
        for x in [c.a, c.b] {
            crate::systems::virt::dump(world, x, "attacked in the chair");
        }
    }
}

/// A live engagement's party who is gone: dead of something else, taken
/// in, cuffed, or under 18.
fn gone(world: &World, x: EntityId) -> bool {
    !crate::systems::law::living(world, x)
        || world.has::<crate::components::Sentence>(x)
        || world.comp::<Brain>(x).is_some_and(|b| b.cuffed_by.is_some())
        || crate::systems::ages::is_protected(world, x)
}

/// Run `e` until it ends (true) or `left` beats are spent (false).
fn advance(world: &mut World, e: &mut Engagement, mut left: u32) -> bool {
    use crate::components::Lod;
    let not_full = |w: &World, x: EntityId| w.comp::<Brain>(x).is_some_and(|b| b.lod != Lod::Full);
    loop {
        if e.duel.is_none() {
            if e.losses[0] >= e.max_losses {
                return true;
            }
            if e.live {
                // A party gone leaves its side; one no longer Full has the
                // rest resolved at once.
                for k in 0..2 {
                    let out: Vec<EntityId> = e.sides[k].iter().copied().filter(|&x| gone(world, x)).collect();
                    for x in out {
                        e.sides[k].retain(|&y| y != x);
                        world.combat.engaged.remove(&x);
                    }
                }
                if e.members().any(|x| not_full(world, x)) {
                    left = u32::MAX;
                }
            }
            let (Some(&a), Some(&b)) = (e.sides[0].first(), e.sides[1].first()) else { return true };
            if let Some(c) = instant(world, a, b, &e.mods) {
                after_duel(world, &c);
                after_clash(world, e, c);
                continue;
            }
            e.duel = Some(duel_start(world, a, b, &e.mods));
        }
        let Some(mut d) = e.duel.take() else { continue };
        if e.live {
            let out: Vec<EntityId> = [d.f[0].id, d.f[1].id].into_iter().filter(|&x| gone(world, x)).collect();
            if !out.is_empty() {
                // The duel is off: the gone leave their sides.
                for x in out {
                    for s in &mut e.sides {
                        s.retain(|&y| y != x);
                    }
                    world.combat.engaged.remove(&x);
                }
                continue;
            }
            if not_full(world, d.f[0].id) || not_full(world, d.f[1].id) {
                left = u32::MAX;
            }
            // One moved out of reach has left the fight (C3a: he fled).
            if d.near && d.f[0].end.is_none() && d.f[1].end.is_none() {
                let (a, b) = (d.f[0].id, d.f[1].id);
                if !crate::systems::law::near(world, a, b, world.config.combat.reach_tiles) {
                    let pos = |x: EntityId| world.comp::<Position>(x).map(|p| (p.tile, p.building));
                    let i = usize::from(pos(a) == Some(d.at[0]));
                    d.f[i].end = Some(End::Fled);
                }
            }
        }
        let before = d.beat;
        let log = if e.live { Some(&mut e.log) } else { None };
        let ended = duel_beats(world, &mut d, &e.mods, &mut left, log);
        e.beats += u32::from(d.beat.saturating_sub(before));
        if !ended {
            e.duel = Some(d);
            return false;
        }
        let (a, b) = (d.f[0].id, d.f[1].id);
        let names = std::mem::take(&mut d.names);
        let out = duel_end(world, d, &e.mods);
        let li = usize::from(out.loser == b);
        let c = Clash {
            a,
            b,
            winner: out.winner,
            loser: out.loser,
            died: out.died,
            end: out.sides[li].end,
            fought: true,
            names,
        };
        after_duel(world, &c);
        after_clash(world, e, c);
    }
}

/// A duel ended: the loser leaves his side (counted), a body that broke and
/// fled runs (C3a: fleeing as movement), the purpose's per-duel tail.
fn after_clash(world: &mut World, e: &mut Engagement, c: Clash) {
    let k = usize::from(!e.sides[0].contains(&c.loser));
    e.sides[k].retain(|&y| y != c.loser);
    e.losses[k] += 1;
    if c.died {
        e.deaths += 1;
    } else {
        e.beaten[k].push(c.loser);
    }
    if e.live {
        world.combat.engaged.remove(&c.loser);
    }
    // Run at the tick's `step` (after exec: a body fleeing its own step's
    // fight would have the step's end move its new plan on).
    if c.end == End::Fled && e.mods.why != Purpose::Bout {
        world.combat.fleeing.push(c.loser);
    }
    let tail = std::mem::replace(&mut e.tail, Tail::Bare);
    match &tail {
        Tail::Pairs { door, place } => crate::systems::raid::pair_end(world, *door, place, &c),
        Tail::Raid(x) => crate::systems::raid::pair_end(world, x.door, &x.place, &c),
        Tail::Arrest { .. } => crate::systems::law::arrest_duel(world, e, &c),
        _ => {}
    }
    e.tail = tail;
    e.clashes.push(c);
}

/// C3a: a body that broke and fled runs from the fight: the flee plan
/// (`FleeToHome`: its Home, else the nearest refuge, `life::refuge`).
fn flee(world: &mut World, id: EntityId) {
    use crate::components::{ActionInstance, GoalKind, Lod};
    let free = crate::systems::law::living(world, id)
        && !world.has::<crate::components::Sentence>(id)
        && !crate::systems::wounded::is_down(world, id)
        && !crate::systems::player::drives(world, id)
        && world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical && b.cuffed_by.is_none() && !b.emigrating);
    if !free {
        return;
    }
    world.abort_plan(id);
    let step = ActionInstance { action: crate::goap::ActionKind::FleeToHome, target: None, tile: None };
    let plan = crate::goap::Plan { goal: GoalKind::Flee, target: None, steps: vec![step], started_tick: world.tick };
    crate::systems::plan::install(world, id, plan);
    let now = world.tick;
    if let Some(b) = world.comp_mut::<Brain>(id) {
        b.current_goal = Some(GoalKind::Flee);
        b.goal_since = now;
    }
    crate::systems::ownership::jobs2_row(world).fled_runs += 1;
}

/// The engagement is over: its members free, its tail run; a held action's
/// verdict left for its next step. Returns the verdict.
fn end(world: &mut World, e: &mut Engagement) -> bool {
    if e.live {
        for x in e.members().collect::<Vec<_>>() {
            world.combat.engaged.remove(&x);
        }
        let ticks = world.tick.saturating_sub(e.start) + 1;
        let row = crate::systems::ownership::jobs2_row(world);
        row.live_fights += 1;
        row.live_fight_ticks += ticks as u32;
    }
    let tail = std::mem::replace(&mut e.tail, Tail::Bare);
    let c = e.result();
    let verdict = match &tail {
        Tail::Bare | Tail::Pairs { .. } => true,
        Tail::Attack { victim } => {
            crate::exec::actions::attack_end(world, c.a, *victim, &c);
            true
        }
        Tail::AttackGuard { victim, guard } => crate::exec::actions::attack_guard_end(world, c.a, *victim, *guard, &c),
        Tail::Abduct { victim } => crate::systems::chrome::abduct_end(world, c.a, *victim, &c),
        Tail::GuardBody { stripper } => c.winner == *stripper,
        Tail::Arrest { berserk } => crate::systems::law::arrest_end(world, e, *berserk),
        Tail::Silence(x) => {
            crate::systems::consequence::silence_end(world, x, &c);
            true
        }
        Tail::Cells(x) => {
            crate::systems::consequence::cell_fight_end(world, x, &c);
            true
        }
        Tail::Reckoning(x) => {
            crate::systems::crew::reckoning_end(world, x, &c);
            true
        }
        Tail::Squat { gname } => {
            crate::systems::gang::squat_end(world, gname, &c);
            true
        }
        Tail::Brawl(x) => {
            crate::systems::misery::brawl_end(world, x, &c);
            true
        }
        Tail::Beating(x) => {
            crate::systems::protection::beating_end(world, x, &c);
            true
        }
        Tail::Scan { tile } => {
            crate::systems::tags::attack_end(world, *tile, &c);
            true
        }
        Tail::Dun(x) => {
            crate::systems::underwater::dun_end(world, x, &c);
            true
        }
        Tail::Confront(x) => {
            crate::systems::underwater::confront_end(world, x, &c);
            true
        }
        Tail::Raid(x) => {
            crate::systems::raid::raid_end(world, x, e);
            true
        }
    };
    if e.live {
        let held = match &tail {
            Tail::Attack { .. } => e.first[0].map(|x| (x, Held::Attack)),
            Tail::AttackGuard { .. } => e.first[0].map(|x| (x, Held::AttackGuard)),
            Tail::Abduct { .. } => e.first[0].map(|x| (x, Held::Abduct)),
            Tail::GuardBody { stripper } => Some((*stripper, Held::GuardBody)),
            _ => None,
        };
        if let Some((x, h)) = held {
            world.combat.done.insert(x, (world.tick, h, verdict));
        }
    }
    e.tail = tail;
    verdict
}

/// A held action's engagement ended: its verdict (taken), when it is
/// `held`'s and fresh (an hour; a body whose plan moved on leaves a stale one).
pub fn take_done(world: &mut World, id: EntityId, held: Held) -> Option<bool> {
    if world.combat.done.is_empty() {
        return None;
    }
    let (t, h, v) = world.combat.done.get(&id).copied()?;
    if h != held {
        return None;
    }
    world.combat.done.remove(&id);
    (world.tick.saturating_sub(t) <= TICKS_PER_HOUR).then_some(v)
}

/// C3a, each tick after exec: every live engagement runs `[combat]
/// beats_per_tick` beats; one that ends runs its tail.
pub fn step(world: &mut World) {
    if !world.combat.done.is_empty() && world.tick.is_multiple_of(TICKS_PER_HOUR) {
        let now = world.tick;
        world.combat.done.retain(|_, x| now.saturating_sub(x.0) <= TICKS_PER_HOUR);
    }
    if world.combat.engagements.is_empty() && world.combat.fleeing.is_empty() {
        return;
    }
    let n = u32::from(world.config.combat.beats_per_tick.max(1));
    let ids: Vec<u32> = world.combat.engagements.keys().copied().collect();
    for id in ids {
        let Some(mut e) = world.combat.engagements.remove(&id) else { continue };
        if advance(world, &mut e, n) {
            end(world, &mut e);
        } else {
            world.combat.engagements.insert(id, e);
        }
    }
    for id in std::mem::take(&mut world.combat.fleeing) {
        flee(world, id);
    }
}

/// Every live engagement run to its end now (before a save: engagements
/// are not saved).
pub fn settle_all(world: &mut World) {
    let ids: Vec<u32> = world.combat.engagements.keys().copied().collect();
    for id in ids {
        let Some(mut e) = world.combat.engagements.remove(&id) else { continue };
        advance(world, &mut e, u32::MAX);
        end(world, &mut e);
    }
}
