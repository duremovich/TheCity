//! M15 § 6 "Competence" and "Poaching" (plan phase 2, W28, W29).
//!
//! A corp's (and the Law's) competence is a number read off its people's
//! skills against the city's adults, rebuilt daily: `comp_ref × (exec_w ×
//! e' + (1 − exec_w) × s')`, where a skill's term is `2 ×` its percentile
//! among the city's adults at seed (`World::skill_quantiles`, `[competence]
//! rank_norm`; else the plan's `skill ÷ city mean`), so the median corp
//! sits at `comp_ref` and its multiplier `clamp(1 + comp_w × (competence −
//! comp_ref), comp_min, comp_max)` at 1. The multiplier scales a corp's
//! Farm output, Lab Data and its Squeeze and Undercut price steps, and the
//! law's binder witness chance and its guards' notice chance. A day-on-day
//! drop of `talent_drop` is a `TalentLost`. Poaching is a hiring rule:
//! once a day a corp under Grow, Research or Secure may make one Persuade
//! move (the resolver's dice) on a rival corp's best worker for its first
//! skilled vacancy. All of it is game state of fictional agents.

use crate::components::{Brain, BuildingKind, Corp, CorpOrder, CorpShock, Job, Law, LawShock, Lod, Role, Skills};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::moves::{MEAN_EXEC, MEAN_FARMING, MEAN_FIGHTING, MEAN_HACKING};
use crate::systems::{law, ownership};
use crate::word::{MoveKind, SocialMove, SocialSkill, Stake, TalentGone};
use crate::world::World;

/// A role's `(MEAN_* slot, value)` pairs (one, or two for a Lab).
type RoleSlots = smallvec::SmallVec<[(usize, f32); 2]>;

/// The one role → skill table (review fix: `role_skill` and `role_term`
/// each carried a copy): a role's scored skills on `s` as `(MEAN_* slot,
/// value)` pairs, one, or two for a Lab's knowledge and hacking, and the
/// label. Spec roles: Farm farming; Market, Bar, Clinic, Garage
/// persuasion; a guard fighting; a Lab knowledge and hacking; a Feed
/// knowledge. `None` for an unskilled role.
fn role_slots(world: &World, role: Role, s: &Skills) -> Option<(RoleSlots, &'static str)> {
    use smallvec::smallvec;
    let k = SocialSkill::Knowledge.index();
    let p = SocialSkill::Persuasion.index();
    Some(match role {
        Role::Farmer => (smallvec![(MEAN_FARMING, s.farming)], "farming"),
        Role::Guard => (smallvec![(MEAN_FIGHTING, s.fighting)], "fighting"),
        Role::Clerk | Role::Bartender | Role::Ripperdoc | Role::Mechanic => {
            (smallvec![(p, s.persuasion)], "persuasion")
        }
        Role::Researcher => (smallvec![(k, s.knowledge), (MEAN_HACKING, s.hacking.max(0.0))], "knowledge"),
        Role::Reporter => (smallvec![(k, s.knowledge)], "knowledge"),
        // L2 L2: the door and the ring fight; the floor persuades; the Fab
        // Tech's production skill is farming (deviation: no mechanical slot).
        Role::Host | Role::Fighter => (smallvec![(MEAN_FIGHTING, s.fighting)], "fighting"),
        Role::Attendant | Role::Cook | Role::Croupier | Role::Concierge => (smallvec![(p, s.persuasion)], "persuasion"),
        Role::Fabber => (smallvec![(MEAN_FARMING, s.farming)], "farming"),
        // M16a (plan C8): a Fixer's office trades on persuasion and knowledge.
        Role::Fixer => (smallvec![(p, s.persuasion), (k, s.knowledge)], "persuasion"),
        // Real economy E26: the kitchen's front persuades.
        Role::Volunteer => (smallvec![(p, s.persuasion)], "persuasion"),
        Role::Gravedigger | Role::Sanitation => return None,
        // Jobs and room J9: a trade's row names its skill.
        Role::Trade(t) => {
            use crate::config::TradeSkill;
            match world.config.trade(t).map_or(TradeSkill::None, |r| r.skill) {
                TradeSkill::None => return None,
                TradeSkill::Farming => (smallvec![(MEAN_FARMING, s.farming)], "farming"),
                TradeSkill::Fighting => (smallvec![(MEAN_FIGHTING, s.fighting)], "fighting"),
                TradeSkill::Persuasion => (smallvec![(p, s.persuasion)], "persuasion"),
                TradeSkill::Knowledge => (smallvec![(k, s.knowledge)], "knowledge"),
                TradeSkill::Hacking => (smallvec![(MEAN_HACKING, s.hacking.max(0.0))], "hacking"),
            }
        }
    })
}

/// A role's skill on `id` (the mean over its slots), against its city
/// mean, and its label. `None` for an unskilled role.
pub fn role_skill(world: &World, id: EntityId, role: Role) -> Option<(f32, f32, &'static str)> {
    let s = world.comp::<Skills>(id)?;
    let (pairs, label) = role_slots(world, role, s)?;
    let n = pairs.len() as f32;
    let v = pairs.iter().map(|&(_, v)| v).sum::<f32>() / n;
    let m = pairs.iter().map(|&(slot, _)| world.skill_means[slot]).sum::<f32>() / n;
    Some((v, m, label))
}

/// One skill's competence term (centred on 1): with `rank_norm`, `2 ×` its
/// percentile among the city's adults at seed (a heavy-tailed skill no
/// longer puts the median corp below 1: phase 2 review); else the plan's
/// `skill ÷ city mean` (W28).
pub fn norm(world: &World, slot: usize, v: f32) -> f32 {
    if world.config.competence.rank_norm {
        if let Some(q) = world.skill_quantiles.get(slot).filter(|q| q.len() >= 2) {
            return 2.0 * crate::systems::moves::percentile(q, v);
        }
    }
    v / world.skill_means.get(slot).copied().unwrap_or(0.25).max(0.01)
}

/// A worker's staff term in its role (`None` for an unskilled role): Farm
/// farming; Market, Bar, Clinic, Garage persuasion; a guard fighting; a
/// Lab the mean of knowledge's and hacking's.
pub fn role_term(world: &World, id: EntityId, role: Role) -> Option<f32> {
    let s = world.comp::<Skills>(id)?;
    let (pairs, _) = role_slots(world, role, s)?;
    Some(pairs.iter().map(|&(slot, v)| norm(world, slot, v)).sum::<f32>() / pairs.len() as f32)
}

/// The exec's term of `e = mean(knowledge, persuasion)`; 0 without one.
fn exec_term(world: &World, exec: Option<EntityId>) -> f32 {
    exec.filter(|&e| law::living(world, e))
        .and_then(|e| world.comp::<Skills>(e))
        .map_or(0.0, |s| norm(world, MEAN_EXEC, 0.5 * (s.knowledge + s.persuasion)))
}

/// The staff term: the mean over skilled staff of their role's term (a
/// staffless corp reads 1).
fn staff_term(world: &World, staff: &[EntityId]) -> f32 {
    let mut sum = 0.0;
    let mut n = 0u32;
    for &a in staff {
        let Some(role) = world.comp::<Job>(a).map(|j| j.role) else { continue };
        if let Some(t) = role_term(world, a, role) {
            sum += t;
            n += 1;
        }
    }
    if n == 0 {
        1.0
    } else {
        sum / n as f32
    }
}

/// W28: a corp's competence now.
pub fn corp_competence(world: &World, corp: EntityId) -> f32 {
    let cfg = &world.config.competence;
    let Some(c) = world.comp::<Corp>(corp) else { return cfg.comp_ref };
    let staff = ownership::employees_of(world, corp);
    let e = exec_term(world, c.exec);
    let s = staff_term(world, &staff);
    (cfg.comp_ref * (cfg.exec_w * e + (1.0 - cfg.exec_w) * s)).clamp(0.0, 1.0)
}

/// The Law's guards on the city payroll.
fn city_guards(world: &World) -> Vec<EntityId> {
    world.guards().iter().copied().filter(|&g| law::is_city_guard(world, g)).collect()
}

/// W28: the Law's competence: `comp_ref × (0.4 × the captain's knowledge
/// term + 0.6 × the guards' mean fighting term)` (no captain: 0; no
/// guards: 1).
pub fn law_competence(world: &World, jail: EntityId) -> f32 {
    let cfg = &world.config.competence;
    let captain = world
        .comp::<Law>(jail)
        .and_then(|l| l.captain)
        .filter(|&c| law::living(world, c))
        .and_then(|c| world.comp::<Skills>(c))
        .map_or(0.0, |s| norm(world, SocialSkill::Knowledge.index(), s.knowledge));
    let guards = city_guards(world);
    let f: Vec<f32> = guards
        .iter()
        .filter_map(|&g| world.comp::<Skills>(g).map(|s| norm(world, MEAN_FIGHTING, s.fighting)))
        .collect();
    let g = if f.is_empty() { 1.0 } else { f.iter().sum::<f32>() / f.len() as f32 };
    (cfg.comp_ref * (0.4 * captain + 0.6 * g)).clamp(0.0, 1.0)
}

/// A competence's multiplier: `clamp(1 + comp_w × (c − comp_ref), comp_min, comp_max)`.
pub fn mult_of(world: &World, c: f32) -> f32 {
    let cfg = &world.config.competence;
    (1.0 + cfg.comp_w * (c - cfg.comp_ref)).clamp(cfg.comp_min, cfg.comp_max)
}

/// W28: the multiplier of a corp or the Law (the Jail's entity); 1 when
/// competence is off or `group` is neither.
pub fn comp_mult(world: &World, group: EntityId) -> f32 {
    if let Some(c) = world.comp::<Corp>(group) {
        return mult_of(world, c.competence);
    }
    if let Some(l) = world.comp::<Law>(group) {
        return mult_of(world, l.competence);
    }
    1.0
}

/// The Law's multiplier (the Jail's), 1 with no Jail.
pub fn law_mult(world: &World) -> f32 {
    world.building_of_kind(BuildingKind::Jail).map_or(1.0, |j| comp_mult(world, j))
}

/// W28: every corp's and the Law's competence set from their people now,
/// with no `TalentLost` (`World::new`, a pre-M15 load).
pub fn seed(world: &mut World) {
    for c in world.corps() {
        let v = corp_competence(world, c);
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.competence = v;
        }
    }
    if let Some(j) = world.building_of_kind(BuildingKind::Jail) {
        let v = law_competence(world, j);
        if let Some(l) = world.comp_mut::<Law>(j) {
            l.competence = v;
        }
    }
}

// ---------------------------------------------------------------------------
// Departures (TalentLost's names)
// ---------------------------------------------------------------------------

fn note(world: &mut World, group: EntityId, who: EntityId, share: f32, skill: &str) {
    let now = world.tick;
    let today = crate::time::day(now);
    let keep = world.talent_gone.get(&group).is_some_and(|g| crate::time::day(g.tick) == today && g.share >= share);
    if !keep {
        world.talent_gone.insert(group, TalentGone { who, tick: now, share, skill: skill.to_string() });
    }
}

/// W28 (`World::vacate_job`): a worker leaving a corp or the Law notes its
/// share of the staff term, `(1 − exec_w) × skill ÷ mean ÷ staff`.
pub fn note_departure(world: &mut World, id: EntityId) {
    let Some(job) = world.comp::<Job>(id) else { return };
    let role = job.role;
    let Some(employer) = job.employer else { return };
    let group = if law::is_city_guard(world, id) {
        world.building_of_kind(BuildingKind::Jail)
    } else {
        world.corp_of_building(employer)
    };
    let Some(group) = group else { return };
    let Some((_, _, label)) = role_skill(world, id, role) else { return };
    let Some(t) = role_term(world, id, role) else { return };
    let staff =
        if world.has::<Corp>(group) { ownership::employees_of(world, group).len() } else { city_guards(world).len() };
    let w = if world.has::<Corp>(group) { 1.0 - world.config.competence.exec_w } else { 0.6 };
    let share = w * t / staff.max(1) as f32;
    note(world, group, id, share, label);
}

/// W28 (`World::kill_by`): a dying corp exec or captain notes its share
/// (`exec_w × e ÷ ē`, the captain's `0.4 × knowledge ÷ ē_k`).
pub fn note_exec_death(world: &mut World, id: EntityId) {
    let Some(s) = world.comp::<Skills>(id).cloned() else { return };
    let corps: Vec<EntityId> =
        world.corps().into_iter().filter(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.exec == Some(id))).collect();
    let exec_w = world.config.competence.exec_w;
    for c in corps {
        let share = exec_w * norm(world, MEAN_EXEC, 0.5 * (s.knowledge + s.persuasion));
        note(world, c, id, share, "knowledge and persuasion");
    }
    if let Some(j) = world.building_of_kind(BuildingKind::Jail) {
        if world.comp::<Law>(j).is_some_and(|l| l.captain == Some(id)) {
            let share = 0.4 * norm(world, SocialSkill::Knowledge.index(), s.knowledge);
            note(world, j, id, share, "knowledge");
        }
    }
}

// ---------------------------------------------------------------------------
// The daily pass
// ---------------------------------------------------------------------------

/// W28, daily in the word's chain: the knowledge a Lab shift teaches, the
/// rust of unused social skills, then every corp's and the Law's
/// competence, with `TalentLost` on a drop of `talent_drop`.
pub fn daily(world: &mut World) {
    knowledge_work(world);
    rust(world);
    let drop = world.config.competence.talent_drop;
    for c in world.corps() {
        let new = corp_competence(world, c);
        let Some(old) = world.comp::<Corp>(c).map(|cc| cc.competence) else { continue };
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.competence = new;
        }
        if old - new >= drop && departed(world, c) {
            ownership::push_corp_shock(world, c, CorpShock::TalentLost);
            talent_lost(world, c, old, new);
        }
    }
    if let Some(j) = world.building_of_kind(BuildingKind::Jail) {
        let new = law_competence(world, j);
        let old = world.comp::<Law>(j).map_or(new, |l| l.competence);
        if let Some(l) = world.comp_mut::<Law>(j) {
            l.competence = new;
        }
        if old - new >= drop && departed(world, j) {
            crate::systems::law_brain::push_shock(world, LawShock::TalentLost);
            talent_lost(world, j, old, new);
        }
    }
    // Records older than a day are spent.
    let now = world.tick;
    world.talent_gone.retain(|_, g| now.saturating_sub(g.tick) <= crate::time::TICKS_PER_DAY);
}

/// Did someone with a competence share die or leave `group` in the last
/// day? (Plan deviation: a drop with no departure, a new captain or exec
/// with less skill, is no `TalentLost`: the spec's event names who died or
/// left.)
fn departed(world: &World, group: EntityId) -> bool {
    let now = world.tick;
    world.talent_gone.get(&group).is_some_and(|g| now.saturating_sub(g.tick) <= crate::time::TICKS_PER_DAY)
}

/// The `TalentLost` event: the departure with the largest share in the
/// last day, killed (a corpse) or left.
fn talent_lost(world: &mut World, group: EntityId, old: f32, new: f32) {
    let gname = if world.has::<Law>(group) { "the Law".to_string() } else { world.owner_label(Some(group)) };
    let Some(g) = world.talent_gone.get(&group).cloned() else { return };
    let how = if law::living(world, g.who) { "left" } else { "killed" };
    let text = format!("{gname} lost {}'s {} ({how}); competence {old:.2} → {new:.2}", world.name_of(g.who), g.skill);
    world.stats.current.word.talent_lost += 1;
    world.push_event(EventKind::TalentLost, &[group, g.who], text);
}

/// W25: `+knowledge_work` per Lab or Feed shift worked yesterday (the
/// shift ledger), at Full and Coarse.
fn knowledge_work(world: &mut World) {
    let step = world.config.skills.knowledge_work;
    let day = u16::try_from(world.day()).unwrap_or(u16::MAX);
    let k = SocialSkill::Knowledge.index();
    let mut workers: Vec<EntityId> = world.workers(Role::Researcher).to_vec();
    workers.extend_from_slice(world.workers(Role::Reporter));
    for r in workers {
        let Some(j) = world.comp::<Job>(r) else { continue };
        let yesterday = j.shift_key_at(world.tick.saturating_sub(1));
        if j.last_shift_day != Some(yesterday) {
            continue;
        }
        if world.comp::<Brain>(r).is_none_or(|b| b.lod == Lod::Statistical) {
            continue;
        }
        if let Some(s) = world.comp_mut::<Skills>(r) {
            s.knowledge = (s.knowledge + step).min(1.0);
            s.last_used[k] = day;
        }
    }
}

/// W25: a Full or Coarse adult's social skill unused for 30 days rusts
/// `skill_rust` a day down toward its seed (`moves::seed_skills`; plan
/// deviation: never up, so an inherited skill keeps its blend).
fn rust(world: &mut World) {
    let step = world.config.skills.skill_rust;
    let day = world.day();
    let mut ids: Vec<EntityId> = world.tier(Lod::Full).to_vec();
    ids.extend_from_slice(world.tier(Lod::Coarse));
    ids.sort_unstable();
    for id in ids {
        let Some(s) = world.comp::<Skills>(id) else { continue };
        let stale: Vec<usize> = (0..4).filter(|&i| day.saturating_sub(u64::from(s.last_used[i])) >= 30).collect();
        if stale.is_empty() || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let seed = crate::systems::moves::seed_skills(world, id);
        if let Some(s) = world.comp_mut::<Skills>(id) {
            let mut v = s.social_all();
            for i in stale {
                if v[i] > seed[i] {
                    v[i] = (v[i] - step).max(seed[i]);
                }
            }
            s.set_social_all(v);
        }
    }
}

// ---------------------------------------------------------------------------
// Poaching (W29)
// ---------------------------------------------------------------------------

/// The roles worth poaching for (W29): Vat Techs, private guards,
/// Researchers, Clerks, Bartenders, Reporters.
fn poachable(world: &World, building: EntityId, role: Role) -> bool {
    match role {
        Role::Farmer | Role::Researcher | Role::Clerk | Role::Bartender | Role::Reporter => true,
        Role::Guard => {
            world.comp::<crate::components::Building>(building).is_some_and(|b| b.kind == BuildingKind::SecurityOffice)
        }
        _ => false,
    }
}

/// W29, daily after the order's act: a corp holding Grow, Research or
/// Secure tries one poach for its first open vacancy in a skilled role.
pub fn poach_daily(world: &mut World, corp: EntityId) {
    let Some(c) = world.comp::<Corp>(corp) else { return };
    if !matches!(c.order, CorpOrder::Grow | CorpOrder::Research | CorpOrder::Secure) {
        return;
    }
    let first = c.buildings.iter().find_map(|&b| {
        world.vacancies.get(&b).and_then(|roles| roles.iter().find(|&&r| poachable(world, b, r)).map(|&r| (b, r)))
    });
    if let Some((b, role)) = first {
        try_poach(world, corp, b, role);
    }
}

/// W29: the best worker in `role` at another corp (max skill, ties the
/// lower id) with skill ≥ `poach_min` and ≥ `poach_gap` above the hire the
/// vacancy would otherwise get (`demography::hire_candidate`); the exec
/// makes a Persuade move with the job at `poach_premium` × the wage. On
/// success the worker moves (the old job's vacancy posted), `Poached`
/// event and deed, `CorpShock::Poached` on the loser.
pub fn try_poach(world: &mut World, corp: EntityId, vacancy: EntityId, role: Role) -> bool {
    let cfg = world.config.competence.clone();
    let Some(exec) = world.comp::<Corp>(corp).and_then(|c| c.exec).filter(|&e| law::living(world, e)) else {
        return false;
    };
    let mut best: Option<(f32, EntityId, EntityId)> = None;
    for &a in world.workers(role) {
        let Some(job) = world.comp::<Job>(a) else { continue };
        let Some(emp) = job.employer else { continue };
        if role == Role::Guard && !law::is_private_guard(world, a) {
            continue;
        }
        let Some(other) = world.corp_of_building(emp).filter(|&o| o != corp) else { continue };
        if a == exec || !law::living(world, a) || world.has::<crate::components::Sentence>(a) {
            continue;
        }
        let Some((v, _, _)) = role_skill(world, a, role) else { continue };
        if v < cfg.poach_min {
            continue;
        }
        if best.is_none_or(|(bv, ba, _)| v > bv || (v == bv && a < ba)) {
            best = Some((v, a, other));
        }
    }
    let Some((v, target, old)) = best else { return false };
    let free = crate::systems::demography::hire_candidate(world, vacancy, role)
        .and_then(|f| role_skill(world, f, role))
        .map_or(0.0, |(fv, _, _)| fv);
    if v < free + cfg.poach_gap {
        return false;
    }
    let wage = (world.config.wage(role) as f32 * cfg.poach_premium).round() as i64;
    let m = SocialMove { actor: exec, target, kind: MoveKind::Persuade, stake: Stake::Job { building: vacancy, wage } };
    if !crate::systems::moves::resolve(world, &m).success {
        return false;
    }
    world.vacate_job(target);
    crate::systems::demography::hire(world, target, vacancy, role);
    if let Some(j) = world.comp_mut::<Job>(target) {
        j.premium = cfg.poach_premium;
    }
    if let Some(v) = world.vacancies.get_mut(&vacancy) {
        if let Some(i) = v.iter().position(|&r| r == role) {
            v.remove(i);
        }
        if v.is_empty() {
            world.vacancies.remove(&vacancy);
        }
    }
    let (pn, an, on) = (world.owner_label(Some(corp)), world.name_of(target), world.owner_label(Some(old)));
    world.push_event(EventKind::Poached, &[corp, target, old], format!("{pn} poached {an} from {on} at {wage}/day"));
    let d = world.district_of_building(vacancy);
    crate::systems::gossip::post_deed(world, d, crate::word::Deed::Poached, Some(corp), Some(old));
    // The old employer's exec knows first-hand.
    if let Some(oe) = world.comp::<Corp>(old).and_then(|c| c.exec).filter(|&e| law::living(world, e)) {
        let deed = crate::word::Deed::Poached;
        let e = crate::components::MemoryEntry {
            subject: Some(corp),
            salience: world.config.gossip.deed_sal.get(deed),
            valence: -world.config.gossip.deed_sev.get(deed),
            deed: Some(deed),
            object: Some(old),
            ..crate::components::MemoryEntry::blank(crate::components::MemoryKind::Rumour, world.tick)
        };
        crate::systems::memory::hear_entry(world, oe, e);
    }
    ownership::push_corp_shock(world, old, CorpShock::Poached);
    world.stats.current.word.poached += 1;
    true
}
