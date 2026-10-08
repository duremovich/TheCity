//! Life pass L2 phase 5: gang desistance (`gang::desist`). An ordinary
//! member walks away at `[living] desist_base` times its terms; the leader
//! never does; a member of a gang in a feud rarely does. Every roll is a
//! `splitmix64` hash over counters.

use citysim::systems::gang;
use citysim::{Brain, Config, EntityId, EventKind, Gang, Job, World, TICKS_PER_DAY};

/// A world on seed 42 with gang 0 holding `n` fresh recruits drawn by
/// `pick` (the gang's own members untouched).
fn setup(n: usize, pick: impl Fn(&World, EntityId) -> bool) -> (World, EntityId, Vec<EntityId>) {
    let mut w = World::new(42, Config::load());
    let g = w.gangs()[0];
    let recruits: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&a| {
            w.has::<Brain>(a)
                && citysim::systems::demography::is_adult(&w, a)
                && w.gang_of(a).is_none()
                && !w.has::<citysim::Sentence>(a)
                && pick(&w, a)
        })
        .take(n)
        .collect();
    assert_eq!(recruits.len(), n, "enough recruits");
    for &a in &recruits {
        gang::enlist(&mut w, a, g);
    }
    (w, g, recruits)
}

/// Run the daily desistance pass for `days` days (the tick advanced by hand).
fn desist_days(w: &mut World, days: u64) {
    for _ in 0..days {
        w.tick += TICKS_PER_DAY;
        gang::desist(w);
    }
}

fn left(w: &World, g: EntityId, who: &[EntityId]) -> usize {
    let members = w.comp::<Gang>(g).map(|x| x.members.clone()).unwrap_or_default();
    who.iter().filter(|a| !members.contains(a)).count()
}

/// An employed member housed outside the Sump: p = base x employed x
/// settled (x aged for some); at base 0.02 that is >= 0.16 a day, so over
/// 10 days most walk away, each with a `GangLeave` "(walked away)" and a
/// JoinGang cooldown.
#[test]
fn test_employed_settled_member_walks_away() {
    let settled = |w: &World, a: EntityId| {
        w.comp::<Job>(a).is_some_and(|j| j.paid_once)
            && w.comp::<citysim::Household>(a)
                .and_then(|h| h.home)
                .is_some_and(|h| !w.district_name(w.district_of_building(h)).contains("Sump"))
    };
    let (mut w, g, recruits) = setup(30, settled);
    w.config.living.desist_base = 0.02;
    w.vendettas.clear();
    desist_days(&mut w, 10);
    let gone = left(&w, g, &recruits);
    // Expected >= 1 - 0.84^10 = 82 % of the ordinary recruits (two may be lieutenants).
    assert!(gone >= 18, "{gone} of 30 employed, settled members walked away in 10 days");
    let walked =
        w.events.iter().filter(|e| e.kind == EventKind::GangLeave && e.text.ends_with("(walked away)")).count();
    assert!(walked >= gone, "every leaver logged ({walked})");
    let gone_one = recruits.iter().copied().find(|a| w.gang_of(*a).is_none()).expect("a leaver");
    let cool = w.comp::<Brain>(gone_one).and_then(|b| b.cooldowns.get(&citysim::GoalKind::JoinGang).copied());
    assert!(cool.is_some_and(|t| t > w.tick), "the leaver cools JoinGang");
}

/// The leader never walks away, however high the rate.
#[test]
fn test_leader_never_walks_away() {
    let (mut w, g, _) = setup(10, |_, _| true);
    w.config.living.desist_base = 1.0;
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader).expect("a leader");
    desist_days(&mut w, 5);
    assert!(w.comp::<Gang>(g).is_some_and(|x| x.members.contains(&leader)), "the leader stays");
}

/// A gang in an open vendetta keeps its members: x `desist_feud` (0.25).
#[test]
fn test_member_in_a_feud_rarely_walks_away() {
    let jobless = |w: &World, a: EntityId| !w.has::<Job>(a);
    let run = |feud: bool| -> usize {
        let (mut w, g, recruits) = setup(80, jobless);
        w.config.living.desist_base = 0.02;
        w.vendettas.clear();
        if feud {
            let other = w.gangs()[1];
            w.vendettas.push(citysim::word::Vendetta {
                a: g,
                b: other,
                since: 0,
                kills: [0, 0],
                w: [1.0, 1.0],
                declared: None,
            });
        }
        desist_days(&mut w, 10);
        left(&w, g, &recruits)
    };
    let (calm, feud) = (run(false), run(true));
    assert!(feud * 2 < calm, "in a feud {feud} walked away, without one {calm}");
}

/// With `[living]` off nothing runs.
#[test]
fn test_no_desistance_with_living_off() {
    let (mut w, g, recruits) = setup(10, |_, _| true);
    w.config.living.desist_base = 1.0;
    w.config.living.enabled = false;
    desist_days(&mut w, 5);
    assert_eq!(left(&w, g, &recruits), 0);
}
