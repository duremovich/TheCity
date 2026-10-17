//! M17 phase 4b (Dylan, 2026-10-13): a gang's succession. "Gangs probably
//! should promote from within or even have a civil war to settle who the
//! new boss is."
//!
//! A decapitated gang (its leader dead or jailed, D36's `split_check`)
//! promotes the top of `gang::leader_ranking`, as before, with a
//! `Succession` event. When the top two stand within `war_margin` of each
//! other and the gang has `war_min_members` free members, the chair is
//! contested: the heir (the ranking's top) holds it, the challenger (the
//! runner-up) disputes it, every free member takes a side (`sides`), and the
//! feud runs on the M15 machinery: each claimant holds a grudge on the
//! other (a lethal Hunt, adopted at once), the strongest seconds of each
//! side hold one on the rival claimant, and the sides are Enemies. A
//! claimant killed or jailed loses the war there; after `war_days` the
//! sides' strength settles it on a keyed roll. The loser dies, flees the
//! city, or leads a splinter through D36's split. Daily and per event:
//! nothing per agent per tick, O(members) per war.

use crate::components::{Brain, CivilWar, Crime, Gang, GoalKind, Lod, Personality, Sentence};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::{gang, grudges, hunt, raid, reputation, social};
use crate::time::TICKS_PER_DAY;
use crate::word::{GrudgeCause, HuntWhy};
use crate::world::World;

/// `econ::hash_unit` purpose of a civil war's settling roll.
const PURPOSE_WAR: u64 = 0x0E17_0004;

/// How a civil war ended for its loser.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Fate {
    Dead,
    Jailed,
    Fled,
    Splinter,
}

/// The succession runs: `[outside.succession] enabled` under the M17 switch.
pub fn on(world: &World) -> bool {
    crate::systems::outside::on(world) && world.config.outside.succession.enabled
}

/// A living, free member of `gang` with a Brain.
fn free_member(world: &World, gang: EntityId, m: EntityId) -> bool {
    crate::systems::law::living(world, m) && world.gang_of(m) == Some(gang) && !world.has::<Sentence>(m)
}

/// A member's pull toward claimant `c`: their edge's affinity, plus
/// `regard_w × (standing + dread × (1 − courage))` of the claimant (M15's
/// reputation: a respected or feared claimant draws the timid).
fn pull(world: &World, m: EntityId, c: EntityId, rep: &crate::word::Reputation, courage: f32) -> f32 {
    let cfg = &world.config.outside.succession;
    let aff = world.edge(m, c).map_or(0.0, |e| e.affinity);
    aff + cfg.regard_w * (rep.standing + rep.dread * (1.0 - courage))
}

/// The free members' sides in a war between `heir` and `challenger`
/// (each claimant on its own side, first; the rest in ranking order). A
/// member backs the challenger only when its pull there beats its pull
/// toward the heir plus `loyalty_w × loyalty` (the order of succession).
pub fn sides(world: &World, gang: EntityId, heir: EntityId, challenger: EntityId) -> (Vec<EntityId>, Vec<EntityId>) {
    let cfg = &world.config.outside.succession;
    let (rh, rc) = (reputation::rep(world, heir), reputation::rep(world, challenger));
    let (mut hs, mut cs) = (vec![heir], vec![challenger]);
    for (m, _) in gang::leader_scores(world, gang) {
        if m == heir || m == challenger {
            continue;
        }
        let (loyalty, courage) = world.comp::<Personality>(m).map_or((0.5, 0.5), |p| (p.loyalty, p.courage));
        let to_heir = pull(world, m, heir, &rh, courage) + cfg.loyalty_w * loyalty;
        let to_challenger = pull(world, m, challenger, &rc, courage);
        if to_challenger > to_heir {
            cs.push(m);
        } else {
            hs.push(m);
        }
    }
    (hs, cs)
}

/// After a decapitation (`gang::run`'s `split_check`). A leader jailed
/// leaves an acting leader (the ranking's top, as before; a `Succession`
/// event, no war: the boss walks back in on release; nothing for a
/// Vagrancy night). A leader dead leaves
/// the chair empty: the top of the ranking is promoted (a `Succession`
/// event), or, when the runner-up stands within `war_margin` of it and the
/// gang has `war_min_members` free members, the chair is contested. True
/// when a war began (D36's split check is then skipped: the war settles
/// who leaves).
pub fn decide(world: &mut World, gang: EntityId, fallen: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let cfg = world.config.outside.succession.clone();
    let Some(g) = world.comp::<Gang>(gang) else { return false };
    let Some(heir) = g.leader else { return false };
    if g.war.is_some() {
        return false;
    }
    let gname = g.name.clone();
    // A Vagrancy night is no interregnum: the boss is back by morning; nor
    // (Shadow V4 bug 8) any term of a day or less (a fare's night handed a
    // gang to its deputy).
    let now = world.tick;
    if world
        .comp::<Sentence>(fallen)
        .is_some_and(|s| s.crime == Crime::Vagrancy || s.until_tick <= now + crate::time::TICKS_PER_DAY)
    {
        return false;
    }
    let dead = !crate::systems::law::living(world, fallen);
    let scores = gang::leader_scores(world, gang);
    let heir_score = scores.iter().find(|x| x.0 == heir).map_or(0.0, |x| x.1);
    let rival = scores.iter().find(|x| x.0 != heir).map(|&(c, s)| (c, heir_score - s));
    if let Some((challenger, gap)) = rival.filter(|_| dead) {
        if scores.len() >= cfg.war_min_members
            && gap < cfg.war_margin
            && start_war(world, gang, heir, challenger, Some(fallen))
        {
            return true;
        }
    }
    let (hn, fnm) = (world.name_of(heir), world.name_of(fallen));
    let text = if dead {
        world.outside.today.successions += 1;
        let ahead = rival.map_or_else(String::new, |(c, gap)| format!(", ahead of {} by {gap:.3}", world.name_of(c)));
        format!("{hn} took over {gname} after {fnm} died{ahead}")
    } else {
        format!("{hn} runs {gname} while {fnm} sits inside")
    };
    world.push_event(EventKind::Succession, &[heir, gang, fallen], text);
    false
}

/// Open the civil war between `heir` (who holds the chair while it runs)
/// and `challenger`: the sides, a `CivilWar` event and a `TookSides` per
/// member, each claimant's grudge on the other (a Hunt taken up at once),
/// the strongest seconds' grudges on the rival claimant, and Enemy edges
/// across the sides. False when either is no free member of `gang` or a
/// war already runs.
pub fn start_war(
    world: &mut World,
    gang: EntityId,
    heir: EntityId,
    challenger: EntityId,
    fallen: Option<EntityId>,
) -> bool {
    let cfg = world.config.outside.succession.clone();
    if heir == challenger || !free_member(world, gang, heir) || !free_member(world, gang, challenger) {
        return false;
    }
    let Some(gname) = world.comp::<Gang>(gang).filter(|g| g.war.is_none()).map(|g| g.name.clone()) else {
        return false;
    };
    let (hs, cs) = sides(world, gang, heir, challenger);
    let now = world.tick;
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.war = Some(CivilWar {
            heir,
            challenger,
            fallen,
            since: now,
            until: now + u64::from(cfg.war_days) * TICKS_PER_DAY,
            heir_side: hs.clone(),
            challenger_side: cs.clone(),
        });
        g.crowned = Some(heir);
    }
    gang::recompute_leader(world, gang);
    world.outside.today.wars += 1;
    let (hn, cn) = (world.name_of(heir), world.name_of(challenger));
    let after = fallen.map_or_else(String::new, |f| format!(" after {} died", world.name_of(f)));
    world.push_event(
        EventKind::CivilWar,
        &[heir, challenger, gang],
        format!("{hn} and {cn} fight for {gname}{after}: {} back {hn}, {} back {cn}", hs.len(), cs.len()),
    );
    for (side, claimant) in [(&hs, heir), (&cs, challenger)] {
        let who = world.name_of(claimant);
        for &m in side.iter().skip(1) {
            let text = format!("{} sided with {who} in the war for {gname}", world.name_of(m));
            world.push_event(EventKind::TookSides, &[m, claimant, gang], text);
        }
    }
    // The feud, on M15's grudges, Hunts and Enemy edges.
    grudges::add(world, heir, challenger, GrudgeCause::Succession, cfg.war_grudge, 0);
    grudges::add(world, challenger, heir, GrudgeCause::Succession, cfg.war_grudge, 0);
    for (side, rival) in [(&hs, challenger), (&cs, heir)] {
        let mut seconds: Vec<EntityId> = side.iter().copied().skip(1).collect();
        raid::by_strength(world, &mut seconds);
        for &s in seconds.iter().take(cfg.war_seconds) {
            grudges::add(world, s, rival, GrudgeCause::Succession, cfg.war_grudge_second, 0);
        }
    }
    let mut pairs = 0;
    'outer: for &a in &hs {
        for &b in &cs {
            if pairs >= cfg.war_enemy_pairs {
                break 'outer;
            }
            social::make_enemy(world, a, b, -0.5);
            pairs += 1;
        }
    }
    for c in [heir, challenger] {
        if !hunt::hunter_ok(world, c) {
            continue;
        }
        if world.comp::<Brain>(c).is_some_and(|b| b.lod == Lod::Statistical) {
            crate::systems::lod::set_lod(world, c, Lod::Coarse);
        }
        hunt::adopt(world, c, HuntWhy::Succession);
    }
    hunt::reindex(world);
    true
}

/// From `gang::recompute_leader_after`: a claimant killed or jailed (a
/// Vagrancy night aside) loses the war at once; the other is crowned. True
/// when it ended a war.
pub fn claimant_fell(world: &mut World, gang: EntityId, fallen: EntityId) -> bool {
    let Some(war) = world.comp::<Gang>(gang).and_then(|g| g.war.clone()) else { return false };
    let winner = if fallen == war.heir {
        war.challenger
    } else if fallen == war.challenger {
        war.heir
    } else {
        return false;
    };
    let fate = match world.comp::<Sentence>(fallen) {
        Some(s) if s.crime == Crime::Vagrancy => return false,
        Some(_) => Fate::Jailed,
        None => Fate::Dead,
    };
    end(world, gang, winner, fallen, fate);
    true
}

/// A war's midnight: a claimant gone from the gang (emigrated, left) loses
/// it; past `until` the sides' strength settles it on a keyed roll (the
/// heir wins under its share of the free sides' `raid::strength`). The
/// loser leads a splinter of its following (`splinter_min` free members
/// left, D36's structure permitting), else flees the city.
pub fn daily(world: &mut World, gang: EntityId) {
    let Some(war) = world.comp::<Gang>(gang).and_then(|g| g.war.clone()) else { return };
    let member = |w: &World, m: EntityId| crate::systems::law::living(w, m) && w.gang_of(m) == Some(gang);
    match (member(world, war.heir), member(world, war.challenger)) {
        (true, true) => {}
        (true, false) => return end(world, gang, war.heir, war.challenger, Fate::Fled),
        (false, true) => return end(world, gang, war.challenger, war.heir, Fate::Fled),
        (false, false) => {
            if let Some(g) = world.comp_mut::<Gang>(gang) {
                g.war = None;
                g.crowned = None;
            }
            return;
        }
    }
    if world.tick < war.until {
        return;
    }
    let strength = |w: &World, side: &[EntityId]| -> f32 {
        side.iter().filter(|&&m| free_member(w, gang, m)).map(|&m| raid::strength(w, m)).sum()
    };
    let (sh, sc) = (strength(world, &war.heir_side), strength(world, &war.challenger_side));
    let p_heir = if sh + sc > 0.0 { sh / (sh + sc) } else { 0.5 };
    let u = crate::systems::econ::hash_unit(world.seed(), PURPOSE_WAR, u64::from(gang.index), war.since);
    let (winner, loser, side) = if u < p_heir {
        (war.heir, war.challenger, &war.challenger_side)
    } else {
        (war.challenger, war.heir, &war.heir_side)
    };
    // The winner takes the chair before the loser goes.
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.war = None;
        g.crowned = Some(winner);
    }
    gang::recompute_leader(world, gang);
    let following: Vec<EntityId> =
        side.iter().copied().filter(|&m| m != loser && m != winner && free_member(world, gang, m)).collect();
    let splinter_min = world.config.outside.succession.splinter_min;
    let fate = if following.len() >= splinter_min
        && free_member(world, gang, loser)
        && gang::split_led(world, gang, loser, &following).is_ok()
    {
        Fate::Splinter
    } else {
        Fate::Fled
    };
    end(world, gang, winner, loser, fate);
}

/// The war is over: `winner` crowned, the counters, `CivilWarEnded`, and a
/// fleeing loser out of the gang and leaving the city.
fn end(world: &mut World, gang: EntityId, winner: EntityId, loser: EntityId, fate: Fate) {
    let gname = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.war = None;
        g.crowned = Some(winner);
    }
    let (wn, ln) = (world.name_of(winner), world.name_of(loser));
    let splinter =
        world.gang_of(loser).filter(|&s| s != gang).and_then(|s| world.comp::<Gang>(s)).map(|s| s.name.clone());
    let t = &mut world.outside.today;
    let text = match fate {
        Fate::Dead => {
            t.war_dead += 1;
            format!("{wn} holds {gname}: {ln} is dead")
        }
        Fate::Jailed => {
            t.war_jailed += 1;
            format!("{wn} took {gname} while {ln} sat in a cell")
        }
        Fate::Splinter => {
            t.war_splinter += 1;
            format!("{wn} won the war for {gname}; {ln} led a splinter away: the {}", splinter.unwrap_or_default())
        }
        Fate::Fled => {
            t.war_fled += 1;
            format!("{wn} won the war for {gname}; {ln} fled the city")
        }
    };
    if fate == Fate::Fled && crate::systems::law::living(world, loser) {
        if world.gang_of(loser) == Some(gang) {
            gang::leave(world, loser, &format!("lost the war for {gname}"));
        }
        let until = world.tick + world.config.living.desist_cooldown_days * TICKS_PER_DAY;
        if let Some(b) = world.comp_mut::<Brain>(loser) {
            b.cooldowns.insert(GoalKind::JoinGang, until);
        }
        if world.comp::<Brain>(loser).is_some_and(|b| !b.emigrating) {
            crate::systems::demography::start_emigrating(world, loser, &format!("lost the war for {gname}"));
        }
    }
    // Jobs & lives phase 7b (a latent M17 bug): a plain recompute here made
    // a loser jailed while it still held the chair the gang's jailed boss;
    // the war's recompute knows the claimant lost it (no boss, no split).
    gang::recompute_leader_after_war(world, gang);
    world.push_event(EventKind::CivilWarEnded, &[winner, loser, gang], text);
}
