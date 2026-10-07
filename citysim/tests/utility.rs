//! M2: response curves, goal selection, think scheduling, mood.

use citysim::utility::curves::Curve;
use citysim::utility::{self, compensate, goals, score_goal, Consideration};
use citysim::{
    Brain, Config, GoalKind, Job, Mood, Needs, Personality, Role, Wallet, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().v1_profile())
}

fn set_needs(w: &mut World, id: citysim::EntityId, f: impl FnOnce(&mut Needs)) {
    f(w.comp_mut::<Needs>(id).expect("needs"));
}

#[test]
fn test_curve_outputs_in_unit_range() {
    let curves = [
        Curve::Linear { m: 2.0, b: -0.5 },
        Curve::Linear { m: -1.0, b: 1.0 },
        Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 },
        Curve::Quadratic { k: 0.5, m: 3.0, c: 0.5, b: -1.0 },
        Curve::Logistic { k: 10.0, mid: 0.75 },
        Curve::Logistic { k: -40.0, mid: 0.1 },
        Curve::Step { t: 0.5, lo: -1.0, hi: 2.0 },
        Curve::Step { t: 1.0, lo: 0.0, hi: 1.0 },
    ];
    for c in curves {
        for i in 0..=100 {
            let x = i as f32 / 100.0;
            let y = c.eval(x);
            assert!((0.0..=1.0).contains(&y), "{c:?} at {x}: {y}");
        }
        assert!((0.0..=1.0).contains(&c.eval(-5.0)));
        assert!((0.0..=1.0).contains(&c.eval(5.0)));
    }
    // spot checks
    assert!((Curve::Logistic { k: 10.0, mid: 0.75 }.eval(0.75) - 0.5).abs() < 1e-5);
    assert_eq!(Curve::Step { t: 1.0, lo: 0.0, hi: 1.0 }.eval(0.999), 0.0);
    assert_eq!(Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 }.eval(0.5), 0.25);
}

#[test]
fn test_hungry_agent_picks_eat() {
    let mut w = world(1);
    let id = w.citizens().into_iter().find(|&id| !w.has::<Job>(id)).expect("an unemployed citizen");
    set_needs(&mut w, id, |n| {
        n.hunger = 0.1;
        n.energy = 1.0;
        n.safety = 1.0;
        n.wealth = 1.0;
        n.belonging = 1.0;
        n.intimacy = 1.0;
    });
    let (goal, trace) = utility::think(&w, id).expect("think");
    assert_eq!(goal, GoalKind::Eat, "{trace:?}");
    assert_eq!(trace.goals[0].goal, GoalKind::Eat);
    assert!(trace.goals.len() <= 5);
}

#[test]
fn test_tired_at_night_picks_sleep() {
    let mut w = world(2);
    let id = w.citizens().into_iter().find(|&id| !w.has::<Job>(id)).expect("an unemployed citizen");
    // 02:00 on day 1
    w.tick = TICKS_PER_DAY + 120;
    set_needs(&mut w, id, |n| {
        n.hunger = 1.0;
        n.energy = 0.2;
        n.safety = 1.0;
        n.wealth = 1.0;
        n.belonging = 1.0;
        n.intimacy = 1.0;
    });
    let (goal, _) = utility::think(&w, id).expect("think");
    assert_eq!(goal, GoalKind::Sleep);
}

#[test]
fn test_hysteresis_prevents_flip() {
    let cs = |v: f32| vec![Consideration::raw("x", v, v)];
    // current goal Idle scores 0.30; a challenger at 0.38 is within 0.10 and loses
    let current = score_goal(GoalKind::Idle, cs(0.30), Some(GoalKind::Idle), 0.10, 0.0).expect("score");
    let challenger = score_goal(GoalKind::Eat, cs(0.38), Some(GoalKind::Idle), 0.10, 0.0).expect("score");
    assert!(current.score > challenger.score, "{} vs {}", current.score, challenger.score);
    // but at 0.41 it wins
    let challenger = score_goal(GoalKind::Eat, cs(0.41), Some(GoalKind::Idle), 0.10, 0.0).expect("score");
    assert!(challenger.score > current.score);
    // any zero consideration makes a goal unavailable
    assert!(score_goal(GoalKind::Eat, vec![Consideration::raw("x", 0.0, 0.0)], None, 0.1, 0.0).is_none());
}

#[test]
fn test_think_runs_every_30_ticks_staggered() {
    let mut w = world(3);
    w.config.lod.force = Some(citysim::Lod::Full);
    let ids = w.citizens();
    w.run_ticks(TICKS_PER_HOUR); // let everyone think at least once
    let before: Vec<u64> = ids.iter().map(|&id| w.comp::<Brain>(id).expect("brain").last_think_tick).collect();
    w.run_ticks(30);
    for (i, &id) in ids.iter().enumerate() {
        let b = w.comp::<Brain>(id).expect("brain");
        assert_eq!(b.last_think_tick, before[i] + 30, "{id}");
        assert_eq!(b.last_think_tick % 30, u64::from(id.index) % 30, "{id} is staggered by index");
        assert!(b.last_think.is_some());
    }
}

#[test]
fn test_worked_example_scores() {
    // Spec › Goal selection › Worked example, reproduced through the scoring
    // primitives with the stated inputs.
    let hysteresis = 0.10;
    let current = Some(GoalKind::Work);
    let wealth = (4.0f32 / (7.0 * 3.0 * 1.5)).clamp(0.0, 1.0);
    let u_wealth = 1.0 - wealth;
    let u_hunger = 0.85f32;

    let eat = score_goal(
        GoalKind::Eat,
        vec![
            Consideration::new("U(hunger)", u_hunger, Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 }),
            Consideration::new("can", 1.0, Curve::Step { t: 1.0, lo: 0.0, hi: 1.0 }),
        ],
        current,
        hysteresis,
        0.0,
    )
    .expect("eat");
    let work = score_goal(
        GoalKind::Work,
        vec![
            Consideration::new("in shift", 1.0, Curve::Step { t: 1.0, lo: 0.0, hi: 1.0 }),
            Consideration::new("energy", 0.6, Curve::Linear { m: 1.0, b: 0.0 }),
            Consideration::new("U(wealth)", u_wealth, Curve::Logistic { k: 8.0, mid: 0.5 }),
            Consideration::new("paid", 1.0, Curve::Step { t: 1.0, lo: 0.0, hi: 1.0 }),
        ],
        current,
        hysteresis,
        0.0,
    )
    .expect("work");
    let earn = score_goal(
        GoalKind::Earn,
        vec![
            Consideration::new("U(wealth)", u_wealth, Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 }),
            Consideration::new("U(hunger)", u_hunger, Curve::Linear { m: 1.0, b: 0.0 }),
            Consideration::new("not in shift", 0.0, Curve::Step { t: 1.0, lo: 0.3, hi: 1.0 }),
            Consideration::new("greed", 0.5, Curve::Linear { m: 0.5, b: 0.5 }),
        ],
        current,
        hysteresis,
        0.0,
    )
    .expect("earn");
    let socialise = score_goal(
        GoalKind::Socialise,
        vec![
            Consideration::new("U(belonging)", 0.5, Curve::Logistic { k: 6.0, mid: 0.5 }),
            Consideration::raw("mood", 0.35, 0.35 * 1.2),
            Consideration::new("sociability", 0.5, Curve::Linear { m: 1.0, b: 0.0 }),
            Consideration::new("phase", 0.0, Curve::Step { t: 1.0, lo: 0.4, hi: 1.0 }),
        ],
        current,
        hysteresis,
        0.0,
    )
    .expect("socialise");

    assert!((work.score - 0.855).abs() < 0.005, "work {}", work.score);
    assert!((eat.score - 0.823).abs() < 0.005, "eat {}", eat.score);
    assert!((earn.score - 0.240).abs() < 0.005, "earn {}", earn.score);
    assert!((socialise.score - 0.072).abs() < 0.005, "socialise {}", socialise.score);
    assert!((compensate(0.7225, 2) - 0.823).abs() < 0.005);
}

#[test]
fn test_worked_example_through_goal_table() {
    // The same example evaluated by the real goal table on a real farmer.
    let mut w = world(4);
    let id =
        w.citizens().into_iter().find(|&id| w.comp::<Job>(id).is_some_and(|j| j.role == Role::Farmer)).expect("farmer");
    let farm = w.comp::<Job>(id).expect("job").employer.expect("farm");
    w.tick = 700; // Work phase, in shift
    w.leave_building(id);
    w.enter_building(id, farm);
    w.comp_mut::<citysim::Market>(w.building_of_kind(citysim::BuildingKind::Market).expect("market"))
        .expect("market")
        .price_food = 3;
    w.comp_mut::<Wallet>(id).expect("wallet").coins = 4;
    w.comp_mut::<citysim::Inventory>(id).expect("inv").food = 0;
    let p = w.comp_mut::<Personality>(id).expect("p");
    p.greed = 0.5;
    p.sociability = 0.5;
    p.lawfulness = 0.6;
    w.comp_mut::<Mood>(id).expect("mood").value = -0.3;
    let b = w.comp_mut::<Brain>(id).expect("brain");
    b.current_goal = Some(GoalKind::Work);
    // hysteresis applies while a plan for the current goal is running
    b.plan = Some(citysim::Plan { goal: GoalKind::Work, target: Some(farm), steps: Vec::new(), started_tick: 0 });
    set_needs(&mut w, id, |n| {
        n.hunger = 0.15;
        n.energy = 0.6;
        n.belonging = 0.499; // just under belonging_satisfied, as the example intends
        n.wealth = (4.0f32 / (7.0 * 3.0 * 1.5)).clamp(0.0, 1.0);
    });
    let (goal, trace) = utility::think(&w, id).expect("think");
    assert_eq!(goal, GoalKind::Work, "{trace:?}");
    let score = |g: GoalKind| trace.goals.iter().find(|s| s.goal == g).map(|s| s.score).expect("in trace");
    assert!((score(GoalKind::Work) - 0.855).abs() < 0.005, "work {}", score(GoalKind::Work));
    assert!((score(GoalKind::Eat) - 0.823).abs() < 0.005, "eat {}", score(GoalKind::Eat));
    assert!((score(GoalKind::Earn) - 0.240).abs() < 0.005, "earn {}", score(GoalKind::Earn));
    assert!((score(GoalKind::Socialise) - 0.072).abs() < 0.005, "socialise {}", score(GoalKind::Socialise));
    assert_eq!(
        goals::GOAL_ORDER.len(),
        25,
        "the spec's 15 goals plus M8's Raid, M11's Found, M12's Squat, M13's Shop, GetHigh, Treat and Loot, M14's Hack, M15's Hunt and GuardBody"
    );
}

#[test]
fn test_mood_in_range() {
    let mut w = world(5);
    for _ in 0..10 {
        w.run_ticks(TICKS_PER_DAY);
        for id in w.citizens() {
            // Children have no Mood.
            let Some(m) = w.comp::<Mood>(id) else { continue };
            assert!((-1.0..=1.0).contains(&m.value), "{id}: {}", m.value);
            assert!(!m.value.is_nan());
        }
    }
    let moved = w.citizens().into_iter().filter(|&id| w.comp::<Mood>(id).is_some_and(|m| m.value != 0.0)).count();
    assert!(moved > 250, "moods should move: {moved} of 300");
}
