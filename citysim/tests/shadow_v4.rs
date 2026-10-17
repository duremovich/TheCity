//! Shadow V4's bug sweep (docs/SHADOW_V4.md "Bugs, ranked"): one test per
//! bug, each failing on the tree the shadow pass read (ce5afe6). The shipped
//! seed-42 city unless a test says otherwise; each calls the mechanism
//! directly with the preconditions set by hand.

use citysim::systems::life;
use citysim::utility::goals::already_satisfied;
use citysim::{Brain, Config, EntityId, GoalKind, Job, World};

fn world() -> World {
    World::new(42, Config::load())
}

/// Jobless free adults with a home, ascending.
fn jobless_housed(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            citysim::systems::law::living(w, a)
                && citysim::systems::demography::is_adult(w, a)
                && !w.has::<Job>(a)
                && !w.has::<citysim::Sentence>(a)
                && life::exec_corp(w, a).is_none()
                && citysim::systems::missions::home_of(w, a).is_some()
                && w.comp::<Brain>(a).is_some_and(|b| !b.emigrating)
        })
        .collect()
}

fn set_coins(w: &mut World, a: EntityId, c: i64) {
    w.comp_mut::<citysim::Wallet>(a).expect("wallet").coins = c;
}

/// Bug 1: with no dole, a jobless adult holding one meal's coins (not
/// broke) still has Earn to plan (Beg, Scavenge); at `earn_meals_no_net`
/// meals it is done; with a dole the old line (broke) stands.
#[test]
fn test_bug1_jobless_with_a_meal_still_earns_without_the_dole() {
    let mut w = world();
    w.levers.dole_per_day = 0;
    let a = *jobless_housed(&w).first().expect("a jobless housed adult");
    // The local Market's price (Earn's savings line's), at least a meal.
    let m = w.local(a, citysim::BuildingKind::Market).expect("a market");
    let price = w.price_for(m, a).max(life::meal_price(&w, a));
    set_coins(&mut w, a, price + 1);
    assert!(!life::broke(&w, a), "a meal's coins is not broke");
    assert!(life::earn_short(&w, a));
    assert!(!already_satisfied(&w, a, GoalKind::Earn, false), "Earn is plannable with one meal's coins");
    let line = w.config.life.earn_meals_no_net * price;
    set_coins(&mut w, a, line);
    assert!(!life::earn_short(&w, a), "days of food in hand: done");
    // With a dole the short line is the broke line.
    w.levers.dole_per_day = 3;
    set_coins(&mut w, a, price + 1);
    assert!(!life::earn_short(&w, a));
}

/// Day workers (every shift from 06:00 on), free and housed, ascending.
fn day_workers(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            citysim::systems::law::living(w, a)
                && !w.has::<citysim::Sentence>(a)
                && citysim::systems::missions::home_of(w, a).is_some()
                && w.comp::<Job>(a).is_some_and(|j| j.employer.is_some() && j.shifts.iter().all(|&(s, _)| s >= 540))
        })
        .collect()
}

/// Bug 2 (Unwind): at 03:00 a day worker's leisure is not "off hours" (it
/// is at 20:00): woken at 23:30 it stood on a corner in the small hours.
#[test]
fn test_bug2_day_worker_night_is_not_off_hours() {
    let mut w = world();
    let day = 3 * citysim::TICKS_PER_DAY;
    let off = |w: &World, a: EntityId| {
        citysim::systems::leisure::considerations(w, a)
            .and_then(|(cs, _)| cs.into_iter().find(|c| c.name == "off hours"))
            .map(|c| c.input)
    };
    w.tick = day + 20 * 60;
    let mut checked = 0;
    for a in day_workers(&w) {
        if let Some(n) = w.comp_mut::<citysim::Needs>(a) {
            n.fun = 0.1;
            n.hunger = 1.0;
            n.energy = 1.0;
        }
        w.tick = day + 20 * 60;
        if off(&w, a) != Some(1.0) {
            continue;
        }
        w.tick = day + 3 * 60;
        assert!(citysim::exec::routine::day_worker_night(&w, a));
        assert_eq!(off(&w, a), Some(0.0), "03:00 is no day worker's leisure hour");
        checked += 1;
        if checked == 5 {
            break;
        }
    }
    assert!(checked > 0, "a day worker with a pick at 20:00");
}

/// Bug 2 (Rest): a doze restores energy only up to `rest_energy_cap`
/// (+0.1 a Rest outran the decay, so a Rest loop never slept).
#[test]
fn test_bug2_rest_restores_only_to_the_cap() {
    let mut w = world();
    let a = *day_workers(&w).first().expect("an agent");
    let cap = w.config.life.rest_energy_cap;
    let rest = |w: &mut World, e: f32| {
        w.comp_mut::<citysim::Needs>(a).expect("needs").energy = e;
        let now = w.tick;
        citysim::exec::actions::on_complete(w, a, citysim::goap::ActionKind::Rest, None, now, now);
        w.comp::<citysim::Needs>(a).expect("needs").energy
    };
    assert!((rest(&mut w, 0.85) - 0.85).abs() < 1e-6, "no gain above the cap");
    assert!((rest(&mut w, 0.3) - 0.4).abs() < 1e-6, "the tired still doze");
    assert!((rest(&mut w, cap - 0.05) - cap).abs() < 1e-6, "up to the cap");
}

/// Free adults outside a gang, ascending.
fn adults(w: &World) -> Vec<EntityId> {
    w.citizens()
        .into_iter()
        .filter(|&a| {
            citysim::systems::law::living(w, a)
                && citysim::systems::demography::is_adult(w, a)
                && !w.has::<citysim::Sentence>(a)
                && w.gang_of(a).is_none()
        })
        .collect()
}

/// `b` stands where `a` stands.
fn beside(w: &mut World, a: EntityId, b: EntityId) {
    let p = w.comp::<citysim::Position>(a).cloned().expect("pos");
    *w.comp_mut::<citysim::Position>(b).expect("pos") = p;
}

fn fight_score(w: &World, a: EntityId) -> Option<f32> {
    citysim::utility::goals::considerations(w, a, GoalKind::Fight, false)
        .and_then(|(cs, flat)| citysim::utility::score_goal(GoalKind::Fight, cs, None, 0.0, flat))
        .map(|s| s.score)
}

/// Bug 3: a lawful, timid, unskilled victim of a robbery (Agathe: Fight
/// 0.43 against ReportCrime 0.39) no longer goes for the robber; a lawless,
/// brave, strong one still does; nobody fights a downed man or wakes to
/// settle a grudge.
#[test]
fn test_bug3_fight_weighs_the_law_the_odds_and_the_body() {
    let mut w = world();
    let ad = adults(&w);
    let (v, r) = (ad[0], ad[1]);
    beside(&mut w, v, r);
    {
        let e = w.edge_entry(v, r);
        e.kind = citysim::RelKind::Enemy;
        e.affinity = -1.0;
    }
    w.remember(v, citysim::MemoryKind::WasRobbed, Some(r), 0.8, -0.8, false);
    let set = |w: &mut World, courage: f32, lawfulness: f32, fighting: f32| {
        let p = w.comp_mut::<citysim::Personality>(v).expect("pers");
        p.courage = courage;
        p.lawfulness = lawfulness;
        w.comp_mut::<citysim::Skills>(v).expect("skills").fighting = fighting;
        w.comp_mut::<citysim::Needs>(v).expect("needs").safety = 0.3;
    };
    w.comp_mut::<citysim::Skills>(r).expect("skills").fighting = 0.4;
    set(&mut w, 0.25, 0.71, 0.15);
    let timid = fight_score(&w, v).expect("Fight scored");
    assert!(timid < 0.39, "the lawful, timid victim reports rather than fights: {timid}");
    set(&mut w, 0.9, 0.1, 0.8);
    let brave = fight_score(&w, v).expect("Fight scored");
    assert!(brave > 0.39, "the lawless, brave, strong victim still fights: {brave}");
    // Asleep: no Fight.
    let now = w.tick;
    w.comp_mut::<Brain>(v).expect("brain").exec =
        citysim::exec::ExecState::Use { kind: citysim::goap::ActionKind::Sleep, until: now + 300, started: now };
    assert_eq!(fight_score(&w, v), None, "nobody wakes to settle a grudge");
    w.comp_mut::<Brain>(v).expect("brain").exec = citysim::exec::ExecState::Idle;
    // Downed: no target.
    assert_eq!(citysim::utility::goals::fight_target(&w, v), Some(r));
    citysim::systems::wounded::down(&mut w, r, None);
    assert_eq!(citysim::utility::goals::fight_target(&w, v), None, "a downed man is not fought");
}

/// Bug 4: a fare dodger who can pay is fined at the Precinct, not jailed,
/// with room in the cells; one who cannot pay serves the night.
#[test]
fn test_bug4_petty_offence_fined_when_the_suspect_can_pay() {
    use citysim::systems::law;
    let mut w = world();
    let guard = w.guards()[0];
    let pool: Vec<EntityId> = adults(&w).into_iter().filter(|&x| !law::is_guard(&w, x)).collect();
    let (a, b) = (pool[0], pool[1]);
    let jailed = w.with::<citysim::Sentence>().len();
    assert!(jailed < usize::from(w.config.buildings.jail.capacity), "room in the cells");
    let fine = w.config.transit.dodge_fine;
    set_coins(&mut w, a, fine + 5);
    law::file_report(&mut w, citysim::Crime::FareEvasion, a, None);
    law::jail_suspect(&mut w, guard, a);
    assert!(!w.has::<citysim::Sentence>(a), "fined, not jailed");
    assert_eq!(w.comp::<citysim::Wallet>(a).expect("w").coins, 5, "the fine paid");
    set_coins(&mut w, b, 0);
    law::file_report(&mut w, citysim::Crime::FareEvasion, b, None);
    law::jail_suspect(&mut w, guard, b);
    assert!(w.has::<citysim::Sentence>(b), "cannot pay: the night in the cells");
}

/// Bug 5 (the office): a corp with an Hq holds office there, wherever the
/// exec lives (distance sorted first and the Hq was not ranked: CEOs held
/// office at Vat Farms).
#[test]
fn test_bug5_exec_office_is_the_hq() {
    let w = world();
    let mut checked = 0;
    for c in w.corps() {
        let Some(cc) = w.comp::<citysim::Corp>(c) else { continue };
        let Some(exec) = cc.exec else { continue };
        let hq = cc
            .buildings
            .iter()
            .copied()
            .find(|&b| w.comp::<citysim::Building>(b).is_some_and(|bd| bd.kind == citysim::BuildingKind::Hq));
        let Some(hq) = hq else { continue };
        assert_eq!(life::hq_of(&w, c, exec), Some(hq), "corp {c:?} holds office at its Hq");
        checked += 1;
    }
    assert!(checked > 0, "a corp with an Hq");
}

/// Bug 5 (street life): an exec is never recruited by a gang and has no
/// Earn to plan (no vehicle theft on a salary), though a desperate
/// non-exec in the same state is recruited.
#[test]
fn test_bug5_exec_kept_out_of_street_earn_and_gangs() {
    use citysim::systems::gang;
    let mut w = world();
    let corp = w.corps().into_iter().find(|&c| w.comp::<citysim::Corp>(c).is_some_and(|cc| cc.exec.is_some()));
    let exec = w.comp::<citysim::Corp>(corp.expect("a corp")).and_then(|cc| cc.exec).expect("an exec");
    let other = *jobless_housed(&w).first().expect("a jobless adult");
    for a in [exec, other] {
        w.comp_mut::<citysim::Needs>(a).expect("needs").hunger = 0.05;
        w.comp_mut::<citysim::Personality>(a).expect("pers").lawfulness = 0.05;
        set_coins(&mut w, a, 0);
    }
    assert!(gang::recruit_gang(&w, other).is_some(), "the control: a desperate jobless adult is taken");
    assert_eq!(gang::recruit_gang(&w, exec), None, "an exec joins no gang");
    assert!(already_satisfied(&w, exec, GoalKind::Earn, false), "an exec has no street Earn");
}

/// Bug 5 (promotion): a corp with no management or professionals to promote
/// takes its fallback exec off the old post (she kept her Guard post).
#[test]
fn test_bug5_fallback_exec_leaves_the_post() {
    use citysim::systems::ownership;
    let mut w = world();
    let corp = w
        .corps()
        .into_iter()
        .find(|&c| {
            !ownership::employees_of(&w, c).is_empty() && w.comp::<citysim::Corp>(c).is_some_and(|cc| cc.exec.is_some())
        })
        .expect("a corp with staff");
    for a in ownership::employees_of(&w, corp) {
        w.comp_mut::<Job>(a).expect("job").grade = 0;
    }
    w.comp_mut::<citysim::Corp>(corp).expect("corp").exec = None;
    w.tick = citysim::TICKS_PER_DAY;
    ownership::run(&mut w);
    let e = w.comp::<citysim::Corp>(corp).and_then(|cc| cc.exec).expect("a new exec");
    assert!(!w.has::<Job>(e), "the new exec left the post");
}
