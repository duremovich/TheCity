//! M15 § 5 and § 6 (plan phase 2, W25, W26): the social skills and the
//! move resolver.
//!
//! A social skill is a number on `Skills` (`0..=1`), seeded once per agent
//! from its own Skill word stream. A move is one seeded dice roll of an
//! actor's skill against a target's resistance, shifted by the two sides'
//! might, reputation gap and taste: `p = clamp(logistic(move_k × (bias +
//! skill_a − resist_t + w_m × might_gap + w_r × rep_gap + w_l × taste)),
//! p_min, p_max)`, one draw on the Move word stream keyed by `(tick, actor,
//! target)`, so the order of callers and the number of world draws in
//! between never change an outcome. Every number here is game state of
//! fictional agents.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use smallvec::SmallVec;

use crate::components::{
    Brain, Kit, Lod, Memory, MemoryEntry, MemoryKind, Personality, Position, RelKind, Sentence, Skills,
};
use crate::config::{MovesCfg, SkillsCfg};
use crate::entity::EntityId;
use crate::systems::{law, reputation};
use crate::word::{MoveKind, MoveOutcome, SocialMove, SocialSkill, Stake, WordNs};
use crate::world::World;

/// `World::skill_means` slots (plan W28): the four social skills in
/// `SocialSkill` order, then farming, fighting, hacking, and the exec term
/// `mean(knowledge, persuasion)`.
pub const MEAN_FARMING: usize = 4;
pub const MEAN_FIGHTING: usize = 5;
pub const MEAN_HACKING: usize = 6;
pub const MEAN_EXEC: usize = 7;

/// Moves are live: the word's master switch and `[moves] enabled`.
pub fn on(world: &World) -> bool {
    world.config.gossip.enabled && world.config.moves.enabled
}

// ---------------------------------------------------------------------------
// Skills (W25)
// ---------------------------------------------------------------------------

/// W25: one draw of the four social skills (`SocialSkill` order) from
/// `rng`: the primary picked by weights `persuasion 1 + sociability`,
/// `intimidation 1 + courage`, `deception 1 + (1 − lawfulness)`,
/// `knowledge 1.5`; the primary at `skill_scale × U^rarity_exp`, the other
/// three at `secondary_scale × U^rarity_exp`, each plus the spec's ±0.05
/// personality tilt, clamped `0..=1`.
pub fn draw_social(cfg: &SkillsCfg, p: Option<&Personality>, rng: &mut ChaCha8Rng) -> [f32; 4] {
    let (soc, cou, law) = p.map_or((0.5, 0.5, 0.5), |p| (p.sociability, p.courage, p.lawfulness));
    let weights = [1.0 + soc, 1.0 + cou, 1.5, 1.0 + (1.0 - law)];
    let total: f32 = weights.iter().sum();
    let u: f32 = rng.random::<f32>() * total;
    let mut acc = 0.0;
    let mut primary = 3;
    for (i, w) in weights.iter().enumerate() {
        acc += w;
        if u < acc {
            primary = i;
            break;
        }
    }
    let tilt = [0.1 * (soc - 0.5), 0.1 * (cou - 0.5), 0.0, 0.1 * (0.5 - law)];
    let mut out = [0.0f32; 4];
    for i in 0..4 {
        let scale = if i == primary { cfg.skill_scale } else { cfg.secondary_scale };
        let v: f32 = rng.random();
        out[i] = (scale * v.powf(cfg.rarity_exp) + tilt[i]).clamp(0.0, 1.0);
    }
    out
}

/// W25: the agent's own seed of the four social skills, re-derived from its
/// Skill word stream (keyed by index and generation, so a reused index draws
/// anew) and its current personality: pure, so nothing extra is saved (the
/// rust target).
pub fn seed_skills(world: &World, id: EntityId) -> [f32; 4] {
    let mut rng = world.rng.word(WordNs::Skill, u64::from(id.index), u64::from(id.generation));
    draw_social(&world.config.skills, world.comp::<Personality>(id), &mut rng)
}

/// Set the four social skills.
pub fn give_social(world: &mut World, id: EntityId, v: [f32; 4]) {
    if let Some(s) = world.comp_mut::<Skills>(id) {
        s.set_social_all(v);
    }
}

/// W25: a newborn's social skills: `inherit_skill ×` the parents' mean
/// plus `(1 − inherit_skill) ×` its own draw, per skill.
pub fn child_social(world: &World, id: EntityId, mother: EntityId, father: EntityId) -> [f32; 4] {
    let own = seed_skills(world, id);
    let parents: Vec<[f32; 4]> =
        [mother, father].iter().filter_map(|&p| world.comp::<Skills>(p).map(|s| s.social_all())).collect();
    if parents.is_empty() {
        return own;
    }
    let k = world.config.skills.inherit_skill;
    let n = parents.len() as f32;
    std::array::from_fn(|i| {
        let mean = parents.iter().map(|p| p[i]).sum::<f32>() / n;
        (k * mean + (1.0 - k) * own[i]).clamp(0.0, 1.0)
    })
}

/// W25/W28 (`World::new`): every agent's social skills from its keyed
/// draw, then the adult city means.
pub fn seed_population(world: &mut World) {
    let ids: Vec<EntityId> = world.with::<Skills>();
    for id in ids {
        let v = seed_skills(world, id);
        give_social(world, id, v);
    }
    world.skill_means = compute_means(world);
    world.skill_quantiles = compute_quantiles(world);
}

/// W44 (`migrate_legacy`): a pre-M15 save's social skills (all four 0)
/// and the means (all 0).
pub fn backfill(world: &mut World) {
    let unset: Vec<EntityId> = world
        .with::<Skills>()
        .into_iter()
        .filter(|&id| world.comp::<Skills>(id).is_some_and(|s| s.social_all() == [0.0; 4]))
        .collect();
    for id in unset {
        let v = seed_skills(world, id);
        give_social(world, id, v);
    }
    if world.skill_means == [0.0; 8] {
        world.skill_means = compute_means(world);
    }
    if world.skill_quantiles.is_empty() {
        world.skill_quantiles = compute_quantiles(world);
    }
}

/// The values of one adult's eight `MEAN_*` slots.
fn slots_of(s: &Skills) -> [f32; 8] {
    [
        s.persuasion,
        s.intimidation,
        s.knowledge,
        s.deception,
        s.farming,
        s.fighting,
        s.hacking.max(0.0),
        0.5 * (s.knowledge + s.persuasion),
    ]
}

/// Phase 2 review: per `MEAN_*` slot, the 101 percentiles (0 %..100 %) of
/// the city's adults (`competence::norm`), computed at seed and on load.
pub fn compute_quantiles(world: &World) -> Vec<Vec<f32>> {
    let mut cols: Vec<Vec<f32>> = vec![Vec::new(); 8];
    // scan-ok: seed and load only
    for id in world.citizens() {
        if !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let Some(s) = world.comp::<Skills>(id) else { continue };
        for (i, v) in slots_of(s).into_iter().enumerate() {
            cols[i].push(v);
        }
    }
    cols.into_iter()
        .map(|mut c| {
            if c.is_empty() {
                return Vec::new();
            }
            c.sort_by(f32::total_cmp);
            let n = c.len() - 1;
            (0..=100).map(|k| c[(k * n + 50) / 100]).collect()
        })
        .collect()
}

/// Where `v` falls in sorted percentiles `q` (`0..=1`): interpolated
/// between neighbours, the middle of a run of equal values.
pub fn percentile(q: &[f32], v: f32) -> f32 {
    let n = q.len();
    if n < 2 {
        return 0.5;
    }
    let lo = q.partition_point(|&x| x < v);
    let hi = q.partition_point(|&x| x <= v);
    let pos = if hi > lo {
        (lo + hi - 1) as f32 / 2.0
    } else if lo == 0 {
        0.0
    } else if lo == n {
        (n - 1) as f32
    } else {
        let (a, b) = (q[lo - 1], q[lo]);
        (lo - 1) as f32 + (v - a) / (b - a).max(1e-9)
    };
    pos / (n - 1) as f32
}

/// W28: the adult city means (`MEAN_*` slots), floored at 0.01 so a ratio
/// never divides by nothing.
pub fn compute_means(world: &World) -> [f32; 8] {
    let mut sum = [0.0f64; 8];
    let mut n = 0u32;
    // scan-ok: seed and load only
    for id in world.citizens() {
        if !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let Some(s) = world.comp::<Skills>(id) else { continue };
        let v = [
            s.persuasion,
            s.intimidation,
            s.knowledge,
            s.deception,
            s.farming,
            s.fighting,
            s.hacking.max(0.0),
            0.5 * (s.knowledge + s.persuasion),
        ];
        for i in 0..8 {
            sum[i] += f64::from(v[i]);
        }
        n += 1;
    }
    std::array::from_fn(|i| if n == 0 { 0.25 } else { ((sum[i] / f64::from(n)) as f32).max(0.01) })
}

/// W8: the speaker's knowledge, 0.5 for everyone while moves are off.
pub fn knowledge(world: &World, id: EntityId) -> f32 {
    if !on(world) {
        return 0.5;
    }
    world.comp::<Skills>(id).map_or(0.5, |s| s.knowledge)
}

/// W25: a successful move sharpens the skill it used (`+skill_drift`) at
/// Full and Coarse (Statistical skills are frozen); the day is noted for
/// the 30-day rust.
pub fn drift(world: &mut World, id: EntityId, s: SocialSkill) {
    let statistical = world.comp::<Brain>(id).is_none_or(|b| b.lod == Lod::Statistical);
    if statistical {
        return;
    }
    let step = world.config.skills.skill_drift;
    let day = u16::try_from(world.day()).unwrap_or(u16::MAX);
    if let Some(sk) = world.comp_mut::<Skills>(id) {
        let v = sk.social_mut(s);
        *v = (*v + step).min(1.0);
        sk.last_used[s.index()] = day;
    }
}

/// The skill a move kind uses.
pub fn skill_of(kind: MoveKind) -> SocialSkill {
    match kind {
        MoveKind::Persuade | MoveKind::Charm => SocialSkill::Persuasion,
        MoveKind::Intimidate => SocialSkill::Intimidation,
        MoveKind::Deceive => SocialSkill::Deception,
    }
}

// ---------------------------------------------------------------------------
// Might (W26)
// ---------------------------------------------------------------------------

/// W26: allies of `id` within `r` Chebyshev tiles (or in the same
/// building): its gang roster and its Friend, Family, Parent and Spouse
/// edges, living and free. Walks the roster and the agent's own edges,
/// never a spatial scan.
pub fn allies_within(world: &World, id: EntityId, r: u32) -> usize {
    let Some(pos) = world.comp::<Position>(id) else { return 0 };
    let (tile, building) = (pos.tile, pos.building);
    let mut seen: SmallVec<[EntityId; 16]> = SmallVec::new();
    let near = |o: EntityId| {
        world
            .comp::<Position>(o)
            .is_some_and(|p| (building.is_some() && p.building == building) || law::chebyshev(p.tile, tile) <= r)
    };
    let ok = |o: EntityId| o != id && law::living(world, o) && !world.has::<Sentence>(o);
    if let Some(g) = world.gang_of(id).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        for &m in &g.members {
            if ok(m) && near(m) && !seen.contains(&m) {
                seen.push(m);
            }
        }
    }
    for o in world.neighbours(id) {
        let kin = world
            .edge(id, o)
            .is_some_and(|e| matches!(e.kind, RelKind::Friend | RelKind::Family | RelKind::Parent | RelKind::Spouse));
        if kin && ok(o) && near(o) && !seen.contains(&o) {
            seen.push(o);
        }
    }
    seen.len()
}

/// W26: `fighting (Skills + Kit) + chrome_might × Kit.visible ÷ 3 +
/// min(ally_might × allies within 8, ally_cap)`.
pub fn might(world: &World, id: EntityId) -> f32 {
    let cfg = &world.config.moves;
    let visible = world.comp::<Kit>(id).map_or(0, |k| k.visible);
    let allies = allies_within(world, id, 8) as f32;
    law::fighting(world, id) + cfg.chrome_might * f32::from(visible) / 3.0 + (cfg.ally_might * allies).min(cfg.ally_cap)
}

// ---------------------------------------------------------------------------
// The resolver (W26)
// ---------------------------------------------------------------------------

/// The five terms of one move besides its kind's bias.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct MoveTerms {
    pub skill_a: f32,
    pub resist_t: f32,
    pub might_gap: f32,
    pub rep_gap: f32,
    /// How the target sees the actor (`reputation::taste(target, actor)`).
    pub taste: f32,
}

/// The kind's bias (`[moves] bias`).
pub fn bias(cfg: &MovesCfg, kind: MoveKind) -> f32 {
    match kind {
        MoveKind::Persuade => cfg.bias.persuade,
        MoveKind::Intimidate => cfg.bias.intimidate,
        MoveKind::Deceive => cfg.bias.deceive,
        MoveKind::Charm => cfg.bias.charm,
    }
}

/// The spec formula, pure (the Move table's unit test).
pub fn move_p(cfg: &MovesCfg, kind: MoveKind, t: &MoveTerms) -> f32 {
    let x = bias(cfg, kind) + t.skill_a - t.resist_t + cfg.w_m * t.might_gap + cfg.w_r * t.rep_gap + cfg.w_l * t.taste;
    let p = 1.0 / (1.0 + (-cfg.move_k * x).exp());
    p.clamp(cfg.p_min, cfg.p_max)
}

/// Spec § 2's meeting gate, pure: a Persuade's target refuses outright when
/// `standing_t − standing_a − meet_taste_w × taste > meet_gap`.
pub fn meeting_refused(meet_gap: f32, meet_taste_w: f32, standing_t: f32, standing_a: f32, taste: f32) -> bool {
    standing_t - standing_a - meet_taste_w * taste > meet_gap
}

fn affinity(world: &World, a: EntityId, b: EntityId) -> f32 {
    world.edge(a, b).map_or(0.0, |e| e.affinity)
}

/// The target's resistance (spec table).
fn resist(world: &World, m: &SocialMove) -> f32 {
    let t = m.target;
    let p = world.comp::<Personality>(t);
    let sk = world.comp::<Skills>(t);
    match m.kind {
        MoveKind::Intimidate => law::courage(world, t) + 0.3 * law::fighting(world, t),
        MoveKind::Persuade => match m.stake {
            Stake::Coins(_) => 1.0 - p.map_or(0.5, |p| p.greed),
            Stake::Info { about } => affinity(world, t, about),
            Stake::Job { .. } => {
                let exec = world.corp_of_agent(t).and_then(|c| world.comp::<crate::components::Corp>(c)?.exec);
                let to_exec = exec.map_or(0.0, |e| affinity(world, t, e).max(0.0));
                0.3 + 0.4 * p.map_or(0.5, |p| p.loyalty) + 0.3 * to_exec - 0.3 * p.map_or(0.5, |p| p.greed)
            }
            Stake::Join(_) => 0.5 - affinity(world, t, m.actor),
        },
        MoveKind::Deceive => sk.map_or(0.0, |s| s.knowledge),
        MoveKind::Charm => 0.5 - affinity(world, t, m.actor),
    }
}

/// The move's terms and whether the meeting gate refuses it (Persuade only).
pub fn terms(world: &World, m: &SocialMove) -> (MoveTerms, bool) {
    let (ra, rt) = (reputation::rep(world, m.actor), reputation::rep(world, m.target));
    let skill_a = world.comp::<Skills>(m.actor).map_or(0.0, |s| s.social(skill_of(m.kind)));
    let rep_gap = match m.kind {
        MoveKind::Intimidate => ra.dread - rt.dread,
        MoveKind::Persuade => ra.standing - rt.standing,
        MoveKind::Deceive | MoveKind::Charm => ra.honour - 0.5,
    };
    let taste = reputation::taste(world, m.target, m.actor);
    let t = MoveTerms {
        skill_a,
        resist_t: resist(world, m),
        might_gap: might(world, m.actor) - might(world, m.target),
        rep_gap,
        taste,
    };
    let tc = &world.config.taste;
    let refused =
        m.kind == MoveKind::Persuade && meeting_refused(tc.meet_gap, tc.meet_taste_w, rt.standing, ra.standing, taste);
    (t, refused)
}

/// W26, pure (tests and the UI): the move's probability and whether the
/// meeting gate refuses it.
pub fn p_of(world: &World, m: &SocialMove) -> (f32, bool) {
    let (t, refused) = terms(world, m);
    (move_p(&world.config.moves, m.kind, &t), refused)
}

/// The Move stream's draw for `m` at the current tick.
pub fn roll(world: &World, m: &SocialMove) -> f32 {
    // The kind in the key's top byte: two kinds of move between the same
    // pair in one tick roll apart.
    let a = world.tick | ((m.kind as u64 + 1) << 56);
    let mut rng = world.rng.word(WordNs::Move, a, (u64::from(m.actor.index) << 32) | u64::from(m.target.index));
    rng.random()
}

/// W26: resolve one move: the meeting gate, one Move-stream draw, then the
/// consequences: a success drifts the actor's skill and leaves a Persuade
/// or Charm target a `Persuaded` memory; a failed Intimidate against a
/// target with courage ≥ `backlash_courage` is backlash (the target turns
/// on the actor: an Enemy edge, `Brain.fight_bonus` for an hour and a
/// `Threatened` memory); a failed Deceive zeroes the target's trust in the
/// actor and posts a `Betrayed` deed. Callers check `on` first.
pub fn resolve(world: &mut World, m: &SocialMove) -> MoveOutcome {
    let (p, refused) = p_of(world, m);
    if refused {
        return MoveOutcome { success: false, p, refused: true, backlash: false };
    }
    let success = roll(world, m) < p;
    let now = world.tick;
    let mut backlash = false;
    if success {
        drift(world, m.actor, skill_of(m.kind));
        if matches!(m.kind, MoveKind::Persuade | MoveKind::Charm) {
            remember_move(world, m.target, MemoryKind::Persuaded, m.actor, 0.3, 0.2);
        }
    } else {
        match m.kind {
            MoveKind::Intimidate => {
                let brave = world
                    .comp::<Personality>(m.target)
                    .is_some_and(|p| p.courage >= world.config.moves.backlash_courage);
                if brave && law::living(world, m.target) {
                    backlash = true;
                    let bonus = world.config.moves.backlash_fight;
                    if let Some(b) = world.comp_mut::<Brain>(m.target) {
                        b.fight_bonus = Some((m.actor, bonus, now + crate::time::TICKS_PER_HOUR));
                    }
                    crate::systems::social::make_enemy(world, m.target, m.actor, -0.3);
                    remember_move(world, m.target, MemoryKind::Threatened, m.actor, 0.5, -0.5);
                }
            }
            MoveKind::Deceive => {
                if world.edge(m.target, m.actor).is_some() {
                    world.edge_entry(m.target, m.actor).trust = 0.0;
                }
                if world.config.gossip.enabled {
                    let d = crate::systems::gossip::home_district(world, m.actor)
                        .unwrap_or_else(|| crate::systems::gossip::talk_district(world, m.actor));
                    crate::systems::gossip::post_deed_at(
                        world,
                        d,
                        crate::word::Deed::Betrayed,
                        Some(m.actor),
                        Some(m.target),
                        0.5,
                    );
                }
            }
            MoveKind::Persuade | MoveKind::Charm => {}
        }
    }
    MoveOutcome { success, p, refused: false, backlash }
}

/// A move's memory on the target: a heard entry (W26), no deed.
fn remember_move(world: &mut World, target: EntityId, kind: MemoryKind, actor: EntityId, salience: f32, valence: f32) {
    if !world.has::<Memory>(target) {
        return;
    }
    let e = MemoryEntry { subject: Some(actor), salience, valence, ..MemoryEntry::blank(kind, world.tick) };
    crate::systems::memory::hear_entry(world, target, e);
}

/// Has the backlash bonus of `id` against `actor` not run out?
pub fn fight_bonus_on(world: &World, id: EntityId, actor: EntityId) -> Option<f32> {
    let (a, bonus, until) = world.comp::<Brain>(id)?.fight_bonus?;
    (a == actor && world.tick < until).then_some(bonus)
}
