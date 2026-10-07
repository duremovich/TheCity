//! M15 phase 2 (plan 2.9): social skills, the move resolver, extortion as
//! Intimidate, competence, poaching and the Purist creed. A move is one
//! seeded dice roll between fictional agents' numbers; these tests check
//! the formula, its keyed draw and what reads it.

use citysim::systems::moves::{self, MoveTerms};
use citysim::systems::{competence, creeds, demography, economy, gang, law};
use citysim::word::{Creed, MoveKind, Reputation, SocialMove, SocialSkill, Stake};
use citysim::{
    Appearance, BuildingKind, Config, Corp, CorpShock, DeathCause, EntityId, EventKind, Gang, Job, Kit, Role, Skills,
    World, TICKS_PER_DAY,
};

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| law::living(w, id) && demography::is_adult(w, id)).collect()
}

fn set_rep(w: &mut World, id: EntityId, dread: f32, standing: f32) {
    let i = id.index as usize;
    if w.reputation.len() <= i {
        w.reputation.resize_with(i + 1, || None);
    }
    w.reputation[i] = Some(Reputation { dread, standing, ..Reputation::default() });
}

fn terms(skill_a: f32, resist_t: f32, might_gap: f32, rep_gap: f32, taste: f32) -> MoveTerms {
    MoveTerms { skill_a, resist_t, might_gap, rep_gap, taste }
}

/// The plan's Move table (W26, `bias.intimidate` 0.8 per W27).
#[test]
fn test_move_table() {
    let full = Config::load();
    let cfg = &full.moves;
    let p = |k, t: MoveTerms| moves::move_p(cfg, k, &t);
    // Intimidate: member intim 0.12, dread 0.1 vs courage 0.5, fighting 0.25, no allies.
    let r1 = p(MoveKind::Intimidate, terms(0.12, 0.5 + 0.3 * 0.25, 0.0, 0.1, 0.0));
    assert!((r1 - 0.8348).abs() < 1e-4, "row 1: {r1}");
    // A feared member: intim 0.5, dread 0.6: clamped at p_max.
    let r2 = p(MoveKind::Intimidate, terms(0.5, 0.575, 0.0, 0.6, 0.0));
    assert_eq!(r2, 0.95, "row 2 clamps");
    // A brave, fighting target with two Family home (might_t + 0.2).
    let r3 = p(MoveKind::Intimidate, terms(0.12, 0.9 + 0.3 * 0.5, -0.2, 0.1, 0.0));
    assert!((r3 - 0.3363).abs() < 1e-4, "row 3: {r3}");
    // Persuade (poach): resist 0.3 + 0.4 × 0.5 + 0.3 × 0.1 − 0.3 × 0.5 = 0.38, standing gap 0.49.
    let r4 = p(MoveKind::Persuade, terms(0.12, 0.38, 0.0, 0.49, 0.0));
    assert!((r4 - 0.5339).abs() < 1e-4, "row 4: {r4}");
    // A Dreg in rags asking an exec for a meeting: refused.
    let tc = &full.taste;
    assert!(moves::meeting_refused(tc.meet_gap, tc.meet_taste_w, 0.63, 0.05, -0.4), "row 5 refused");
    // Deceive (AskAround): deception 0.3 vs knowledge 0.12, honour 0.5.
    let r6 = p(MoveKind::Deceive, terms(0.3, 0.12, 0.0, 0.0, 0.0));
    assert!((r6 - 0.6726).abs() < 1e-4, "row 6: {r6}");
}

/// W26: the draw is keyed on `(tick, actor, target)`: two calls in either
/// order, with world draws in between, give the same outcomes; p stays in
/// `p_min..=p_max`.
#[test]
fn test_resolve_order_independent_and_clamped() {
    use rand::Rng;
    let mut w = World::new(42, Config::load());
    w.tick = 3 * TICKS_PER_DAY + 17;
    let ads = adults(&w);
    let m1 = SocialMove { actor: ads[0], target: ads[1], kind: MoveKind::Intimidate, stake: Stake::Coins(3) };
    let m2 = SocialMove { actor: ads[2], target: ads[3], kind: MoveKind::Charm, stake: Stake::Coins(0) };
    let mut a = w.clone();
    let mut b = w.clone();
    let a1 = moves::resolve(&mut a, &m1);
    for _ in 0..100 {
        let _: f32 = a.rng.world().random();
    }
    let a2 = moves::resolve(&mut a, &m2);
    let b2 = moves::resolve(&mut b, &m2);
    for _ in 0..37 {
        let _: f32 = b.rng.world().random();
    }
    let b1 = moves::resolve(&mut b, &m1);
    assert_eq!(a1, b1, "same outcome either order");
    assert_eq!(a2, b2, "same outcome either order");
    for i in 0..60 {
        let m = SocialMove {
            actor: ads[i],
            target: ads[ads.len() - 1 - i],
            kind: [MoveKind::Persuade, MoveKind::Intimidate, MoveKind::Deceive, MoveKind::Charm][i % 4],
            stake: Stake::Coins(5),
        };
        let (p, _) = moves::p_of(&w, &m);
        assert!((0.05..=0.95).contains(&p), "p {p} clamped");
    }
}

/// W27: 1,000 keyed trials of the same Shakedown at dread 0.1 and 0.6.
#[test]
fn test_higher_dread_extorter_succeeds_more() {
    let mut w = World::new(43, Config::load());
    let ads = adults(&w);
    let (actor, target) = (ads[5], ads[6]);
    let m = SocialMove { actor, target, kind: MoveKind::Intimidate, stake: Stake::Coins(3) };
    let mut wins = [0u32; 2];
    for (k, dread) in [0.1f32, 0.6].into_iter().enumerate() {
        set_rep(&mut w, actor, dread, 0.2);
        let (p, _) = moves::p_of(&w, &m);
        for t in 0..1000u64 {
            w.tick = t * 7 + 1;
            if moves::roll(&w, &m) < p {
                wins[k] += 1;
            }
        }
    }
    assert!(wins[1] > wins[0], "the feared succeed more: {wins:?}");
}

/// An exec (Corp audience) and a Dreg (no Home, no job) in `dress`.
fn exec_and_dreg(dress: u8) -> (World, EntityId, EntityId) {
    let mut w = World::new(44, Config::load());
    let exec = w.corps().into_iter().find_map(|c| w.comp::<Corp>(c).and_then(|cc| cc.exec)).expect("an exec");
    let dreg = adults(&w)
        .into_iter()
        .find(|&a| a != exec && !w.has::<Job>(a) && w.gang_of(a).is_none())
        .expect("a jobless adult");
    w.set_home(dreg, None);
    w.insert(dreg, Appearance { dress, chrome: 0, colours: None, dress_pin: None });
    set_rep(&mut w, exec, 0.0, 0.63);
    set_rep(&mut w, dreg, 0.0, 0.05);
    (w, exec, dreg)
}

/// Spec § 2's meeting gate: a Dreg in rags gets no meeting with an exec.
#[test]
fn test_meeting_gate_refuses_dreg_persuade_on_exec() {
    let (mut w, exec, dreg) = exec_and_dreg(0);
    let m = SocialMove { actor: dreg, target: exec, kind: MoveKind::Persuade, stake: Stake::Coins(5) };
    let (_, refused) = moves::p_of(&w, &m);
    assert!(refused, "taste {}", citysim::systems::reputation::taste(&w, exec, dreg));
    let out = moves::resolve(&mut w, &m);
    assert!(out.refused && !out.success);
}

/// The same Dreg in a Spire suit (dress 3) is not refused: the gap ≤ 0.5.
#[test]
fn test_suit_opens_some_meetings() {
    let (w, exec, dreg) = exec_and_dreg(3);
    let m = SocialMove { actor: dreg, target: exec, kind: MoveKind::Persuade, stake: Stake::Coins(5) };
    let (_, refused) = moves::p_of(&w, &m);
    assert!(!refused, "taste {}", citysim::systems::reputation::taste(&w, exec, dreg));
}

/// W25: one primary social skill per adult: any skill ≥ 0.8 for 4-7 %,
/// each skill ≥ 0.5 for 6-11 %, no secondary ≥ 0.65.
#[test]
fn test_skill_rarity_seed() {
    let w = World::new(42, Config::load());
    let skills: Vec<[f32; 4]> =
        adults(&w).into_iter().filter_map(|a| w.comp::<Skills>(a).map(|s| s.social_all())).collect();
    assert!(skills.len() > 1500, "{} adults", skills.len());
    let n = skills.len() as f32;
    let rare = skills.iter().filter(|s| s.iter().any(|&v| v >= 0.8)).count() as f32 / n;
    assert!((0.04..=0.07).contains(&rare), "any-skill >= 0.8 share {rare}");
    for k in 0..4 {
        let half = skills.iter().filter(|s| s[k] >= 0.5).count() as f32 / n;
        assert!((0.06..=0.11).contains(&half), "{:?} >= 0.5 share {half}", SocialSkill::ALL[k]);
    }
    for s in &skills {
        let mut v = *s;
        v.sort_by(|a, b| b.total_cmp(a));
        assert!(v[1] < 0.65, "a secondary at {}", v[1]);
    }
}

/// W28: competence is normalised by the city means, so the Farm output
/// multiplier lands neutral: the farm-weighted mean multiplier at seed,
/// averaged over 20 seeds, within ±3 % of 1 (one seed's three Food execs
/// swing it ±10 %: the exec term is a heavy-tailed skill by design).
#[test]
fn test_competence_neutral_on_landing() {
    let mut sum = 0.0f32;
    let n = 20;
    for seed in 0..n {
        let w = World::new(1000 + seed, Config::load());
        let farms: Vec<EntityId> = w.buildings_of_kind(BuildingKind::Farm).to_vec();
        let m: f32 =
            farms.iter().map(|&f| w.corp_of_building(f).map_or(1.0, |c| competence::comp_mult(&w, c))).sum::<f32>()
                / farms.len() as f32;
        sum += m;
    }
    let mean = sum / n as f32;
    assert!((mean - 1.0).abs() <= 0.03, "landing multiplier {mean}");
    // And with competence off the multiplier is exactly 1.
    let mut off = Config::load();
    off.competence.enabled = false;
    let w = World::new(1000, off);
    for c in w.corps() {
        assert_eq!(competence::comp_mult(&w, c), 1.0);
    }
}

/// Σ food the Farms produced over `days`: each tick's rise in a Farm's
/// stock plus its production accumulator (a haul only lowers it).
fn farm_credit(mut w: World, days: u64) -> f64 {
    let farms: Vec<EntityId> = w.buildings_of_kind(BuildingKind::Farm).to_vec();
    let level = |w: &World, f: EntityId| {
        w.comp::<citysim::Building>(f).map_or(0.0, |b| f64::from(b.stock_food) + f64::from(b.production_accum))
    };
    let mut made = 0.0;
    for _ in 0..days * TICKS_PER_DAY {
        let before: Vec<f64> = farms.iter().map(|&f| level(&w, f)).collect();
        w.tick();
        for (i, &f) in farms.iter().enumerate() {
            made += (level(&w, f) - before[i]).max(0.0);
        }
    }
    made
}

/// W28, the plan's mechanism check: on seed 42, 10 days of Farm output with
/// competence on within ±3 % of `comp_w = 0` (seed 42 +1.5 %; seeds 1, 43,
/// 44 read +2.8 %, −3.0 %, +0.3 %: a weak Food exec still costs output).
#[test]
fn test_competence_landing_farm_credit_seed_42() {
    let on = farm_credit(World::new(42, Config::load()), 10);
    let mut flat = Config::load();
    flat.competence.comp_w = 0.0;
    let off = farm_credit(World::new(42, flat), 10);
    let r = on / off;
    eprintln!("10-day farm credit {on:.0} vs comp_w 0 {off:.0}: ratio {r:.4}");
    assert!((r - 1.0).abs() <= 0.03, "10-day farm credit {on:.0} vs comp_w 0 {off:.0}: ratio {r:.4}");
}

/// W28: kill a skilled exec: the next midnight's competence is lower and a
/// `TalentLost` names the dead.
#[test]
fn test_competence_falls_after_exec_dies() {
    let mut w = World::new(45, Config::load());
    w.run_ticks(TICKS_PER_DAY + 30);
    let corp = w
        .corps()
        .into_iter()
        .find(|&c| w.comp::<Corp>(c).and_then(|cc| cc.exec).is_some_and(|e| law::living(&w, e)))
        .expect("a corp with an exec");
    let exec = w.comp::<Corp>(corp).and_then(|c| c.exec).expect("exec");
    if let Some(s) = w.comp_mut::<Skills>(exec) {
        s.knowledge = 1.0;
        s.persuasion = 1.0;
    }
    competence::seed(&mut w);
    let before = w.comp::<Corp>(corp).map(|c| c.competence).expect("corp");
    let first_event = w.next_event_id;
    w.kill_by(exec, DeathCause::Violence, None);
    // The successor is the corp's greediest employee: make every candidate unskilled.
    for a in citysim::systems::ownership::employees_of(&w, corp) {
        if let Some(s) = w.comp_mut::<Skills>(a) {
            s.knowledge = 0.0;
            s.persuasion = 0.0;
        }
    }
    let to_midnight = TICKS_PER_DAY - w.tick % TICKS_PER_DAY + 2;
    w.run_ticks(to_midnight);
    let after = w.comp::<Corp>(corp).map(|c| c.competence).expect("corp");
    assert!(after < before, "competence {before} -> {after}");
    let named = w
        .events
        .iter()
        .filter(|e| e.id >= first_event && e.kind == EventKind::TalentLost)
        .any(|e| e.actors.first() == Some(&corp) && e.actors.get(1) == Some(&exec));
    assert!(named, "a TalentLost names the dead exec");
}

/// W29: a rival corp's best Clerk is poached for a vacancy: the job moves,
/// the premium is paid at the next collection, the loser is shocked.
#[test]
fn test_poach_moves_job_and_shocks_loser() {
    let mut w = World::new(46, Config::load());
    // The target: a corp Clerk; the poacher: another corp with a Market and an exec.
    let target = w
        .workers(Role::Clerk)
        .iter()
        .copied()
        .find(|&a| w.comp::<Job>(a).and_then(|j| j.employer).and_then(|e| w.corp_of_building(e)).is_some())
        .expect("a corp Clerk");
    let old_market = w.comp::<Job>(target).and_then(|j| j.employer).expect("employer");
    let old = w.corp_of_building(old_market).expect("old corp");
    let (corp, market) = w
        .buildings_of_kind(BuildingKind::Market)
        .iter()
        .copied()
        .find_map(|m| {
            w.corp_of_building(m)
                .filter(|&c| c != old && w.comp::<Corp>(c).and_then(|cc| cc.exec).is_some())
                .map(|c| (c, m))
        })
        .expect("a rival corp Market");
    let exec = w.comp::<Corp>(corp).and_then(|c| c.exec).expect("exec");
    if let Some(s) = w.comp_mut::<Skills>(target) {
        s.persuasion = 1.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(exec) {
        s.persuasion = 1.0;
    }
    w.vacancies.insert(market, vec![Role::Clerk]);
    if let Some(free) = demography::hire_candidate(&w, market, Role::Clerk) {
        if let Some(s) = w.comp_mut::<Skills>(free) {
            s.persuasion = 0.0;
        }
    }
    let mut ok = false;
    for t in 0..400u64 {
        w.tick = 2 * TICKS_PER_DAY + t;
        if competence::try_poach(&mut w, corp, market, Role::Clerk) {
            ok = true;
            break;
        }
    }
    assert!(ok, "the poach succeeds within 400 tries");
    let job = w.comp::<Job>(target).cloned().expect("hired");
    assert_eq!(job.employer, Some(market));
    let premium = w.config.competence.poach_premium;
    assert_eq!(job.premium, premium);
    assert!(w.comp::<Corp>(old).is_some_and(|c| c.shocks.contains(&CorpShock::Poached)), "the loser is shocked");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Poached && e.actors[1] == target));
    // The next collection pays the premium.
    if let Some(j) = w.comp_mut::<Job>(target) {
        j.days_unpaid = 1;
    }
    if let Some(c) = w.comp_mut::<Corp>(corp) {
        c.treasury += 1000;
    }
    let paid = economy::collect_wage(&mut w, target);
    let wage = w.config.economy.wage(Role::Clerk) as f32;
    let floor = wage * premium * (1.0 - w.levers.tax_rate) - 1.0;
    assert!(paid as f32 >= floor && paid as f32 > wage * (1.0 - w.levers.tax_rate), "paid {paid} (wage {wage})");
}

/// The Unplugged, with one member.
fn unplugged_world(seed: u64) -> (World, EntityId) {
    let w = World::new(seed, Config::load());
    let g = w.gang_list().iter().copied().find(|&g| creeds::is_purist(&w, g)).expect("The Unplugged");
    (w, g)
}

/// W31: a Purist member who installs chrome is cast out (the install hook).
#[test]
fn test_purist_expels_chromed_member() {
    let (mut w, g) = unplugged_world(47);
    let hq = w.comp::<Gang>(g).map(|x| x.hideout).expect("hideout");
    assert_eq!(w.comp::<citysim::Building>(hq).and_then(|b| b.label.clone()).as_deref(), Some("Chapel"));
    let member = adults(&w).into_iter().find(|&a| w.gang_of(a).is_none()).expect("an adult");
    gang::enlist(&mut w, member, g);
    creeds::on_install(&mut w, member);
    assert_eq!(w.gang_of(member), Some(g), "unchromed: kept");
    w.insert(member, Kit { visible: 1, chrome: true, ..Kit::default() });
    creeds::on_install(&mut w, member);
    assert_eq!(w.gang_of(member), None, "chromed: cast out");
    assert!(w.events.iter().any(|e| e.kind == EventKind::Expelled && e.actors[0] == member));
    // SetCreed on another gang expels its chromed at once.
    let other = w.gang_list().iter().copied().find(|&x| x != g).expect("another gang");
    let chromed = adults(&w).into_iter().find(|&a| w.gang_of(a).is_none() && a != member).expect("adult");
    gang::enlist(&mut w, chromed, other);
    w.insert(chromed, Kit { visible: 2, chrome: true, ..Kit::default() });
    w.push_command(citysim::PlayerCommand::SetCreed { gang: other, creed: Some(Creed::Purist) });
    w.tick();
    assert_eq!(w.gang_of(chromed), None, "SetCreed Purist expels the chromed");
}

/// W31: a Purist gang refuses a chromed recruit the edge would have brought.
#[test]
fn test_purist_refuses_chromed_recruit() {
    let (mut w, g) = unplugged_world(48);
    let ads = adults(&w);
    let member = ads.iter().copied().find(|&a| w.gang_of(a).is_none()).expect("member");
    gang::enlist(&mut w, member, g);
    let recruit = ads
        .iter()
        .copied()
        .find(|&a| a != member && w.gang_of(a).is_none() && w.neighbours(a).all(|o| w.gang_of(o).is_none()))
        .expect("a recruit with no gang ties");
    let e = w.edge_entry(recruit, member);
    e.affinity = 0.95;
    e.trust = 0.9;
    e.kind = citysim::RelKind::Friend;
    assert_eq!(gang::recruit_gang(&w, recruit), Some(g), "unchromed: the Friend edge brings them in");
    w.insert(recruit, Kit { visible: 1, chrome: true, ..Kit::default() });
    assert_ne!(gang::recruit_gang(&w, recruit), Some(g), "chromed: refused");
}

/// W25: the skill draws are keyed word streams: the world stream does not
/// move, the same id draws the same, another id draws anew.
#[test]
fn test_skill_draw_untouches_world_stream() {
    let mut w = World::new(49, Config::load());
    let ads = adults(&w);
    let before = format!("{:?}", w.rng);
    let a = moves::seed_skills(&w, ads[0]);
    let a2 = moves::seed_skills(&w, ads[0]);
    let b = moves::seed_skills(&w, ads[1]);
    moves::give_social(&mut w, ads[2], a);
    let _ = moves::child_social(&w, ads[3], ads[0], ads[1]);
    assert_eq!(before, format!("{:?}", w.rng), "the world stream is untouched");
    assert_eq!(a, a2);
    assert_ne!(a, b);
    assert_eq!(w.comp::<Skills>(ads[0]).map(|s| s.social_all()), Some(a), "seeded at World::new from the same draw");
}
