//! M9: the law as a faction. The captain, the posture brain, the duty split,
//! crackdown loops, the player's pin.

use citysim::systems::law_brain::{self, PostureInputs};
use citysim::systems::{gang, law};
use citysim::{
    Building, BuildingKind, Config, Crime, EntityId, EventKind, Gang, LawShock, Personality, PlayerCommand, Posture,
    World, TICKS_PER_DAY,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load())
}

fn civilians(w: &World, n: usize) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| !w.has::<citysim::Job>(id) && w.has::<citysim::Brain>(id)).take(n).collect()
}

fn guards(w: &World) -> Vec<EntityId> {
    law_brain::guards(w)
}

fn inputs() -> PostureInputs {
    PostureInputs {
        wanted_gang: None,
        pressure: 0.0,
        jailed_gang: 0.0,
        breakout_recent: false,
        guards: 10,
        hardened: false,
        courage: 0.5,
        lawfulness: 0.7,
    }
}

fn best(i: &PostureInputs) -> Posture {
    law_brain::score_postures(i, &Config::load().law)[0].posture
}

#[test]
fn test_posture_patrol_at_rest_and_crackdown_under_pressure() {
    assert_eq!(best(&inputs()), Posture::Patrol);
    let some = EntityId { index: 1, generation: 0 };
    let pressed = PostureInputs { wanted_gang: Some(some), pressure: 1.0, jailed_gang: 0.25, ..inputs() };
    assert_eq!(best(&pressed), Posture::Crackdown);
    let mild = PostureInputs { pressure: 0.4, ..pressed.clone() };
    assert_eq!(best(&mild), Posture::Patrol, "a few reports are the routine");
    let short_handed = PostureInputs { guards: 1, ..pressed.clone() };
    assert_eq!(best(&short_handed), Posture::Patrol, "one guard cannot crack down");
    let nobody = PostureInputs { wanted_gang: None, ..pressed };
    assert_ne!(best(&nobody), Posture::Crackdown, "no gang to crack down on");
}

#[test]
fn test_posture_garrison_after_a_jailbreak_or_a_full_jail() {
    let after = PostureInputs { breakout_recent: true, ..inputs() };
    assert_eq!(best(&after), Posture::Garrison);
    let full = PostureInputs { jailed_gang: 0.75, courage: 0.2, ..inputs() };
    assert_eq!(best(&full), Posture::Garrison, "a timid captain sits on a Jail full of gang members");
    let brave = PostureInputs { jailed_gang: 0.4, courage: 0.9, ..inputs() };
    assert_eq!(best(&brave), Posture::Patrol);
}

#[test]
fn test_captain_is_the_most_lawful_guard() {
    let mut w = world(60);
    let gs = guards(&w);
    assert!(gs.len() >= 2);
    for (i, &g) in gs.iter().enumerate() {
        w.comp_mut::<Personality>(g).expect("p").lawfulness = 0.3 + 0.01 * i as f32;
    }
    let top = *gs.last().expect("a guard");
    assert_eq!(law_brain::recompute_captain(&mut w), Some(top));
    assert_eq!(w.law().and_then(|l| l.captain), Some(top));
    // A dead captain is replaced at the next rescoring.
    w.kill(top, citysim::DeathCause::Violence);
    assert!(w.law().is_some_and(|l| l.shocks.contains(&LawShock::GuardKilled)));
    law_brain::rescore(&mut w, 0.0, "test");
    let captain = w.law().and_then(|l| l.captain).expect("a new captain");
    assert_ne!(captain, top);
    assert!(law::is_guard(&w, captain));
}

#[test]
fn test_wanted_gang_counts_reports_and_skips_the_bribed() {
    let mut w = world(61);
    let gs = w.gangs();
    let civ = civilians(&w, 2);
    gang::enlist(&mut w, civ[0], gs[0]);
    gang::enlist(&mut w, civ[1], gs[1]);
    for crime in [Crime::Theft, Crime::Extortion, Crime::Assault] {
        law::file_report(&mut w, crime, civ[0], None);
    }
    law::file_report(&mut w, Crime::Theft, civ[1], None);
    law::file_report(&mut w, Crime::Theft, civ[1], None); // a refresh, not a new report
    assert_eq!(law_brain::reports_by_gang(&w).get(&gs[0]), Some(&3));
    assert_eq!(law_brain::reports_by_gang(&w).get(&gs[1]), Some(&1));
    assert_eq!(law_brain::wanted_gang(&w), Some((gs[0], 3)));
    w.comp_mut::<Gang>(gs[0]).expect("g").bribe_until = Some(w.tick + 1000);
    assert_eq!(law_brain::wanted_gang(&w), Some((gs[1], 1)), "a bribed gang is left alone");
    // Reports age out of the window.
    w.tick += (w.config.law.window_days + 1) * TICKS_PER_DAY;
    assert_eq!(law_brain::wanted_gang(&w), None);
}

#[test]
fn test_jail_duty_per_posture() {
    let mut w = world(62);
    let gs = guards(&w);
    let key = 4i64;
    let on_jail = |w: &World| gs.iter().filter(|&&g| law::jail_duty(w, g, key)).count();
    let patrol = on_jail(&w);
    assert!(patrol >= 1 && patrol < gs.len(), "Patrol: a split ({patrol} of {})", gs.len());
    for &g in &gs {
        assert_eq!(law::jail_duty(&w, g, key), law::jail_day(g, key));
    }
    let gang = w.gangs()[0];
    {
        let l = w.law_mut().expect("law");
        l.posture = Posture::Crackdown;
        l.target = Some(gang);
    }
    let crackdown = on_jail(&w);
    assert!(crackdown < patrol, "Crackdown: fewer hold the Jail ({crackdown} vs {patrol})");
    w.law_mut().expect("law").target = None;
    assert_eq!(on_jail(&w), patrol, "a Crackdown on nobody is a Patrol");
    w.law_mut().expect("law").posture = Posture::Garrison;
    assert_eq!(on_jail(&w), gs.len(), "Garrison: everyone");
    assert!(law::garrisoned(&w));
}

#[test]
fn test_crackdown_patrol_loop_walks_the_target_turf() {
    let mut w = world(63);
    let gang = w.gangs()[0];
    let hideout = w.hideout_of(gang).expect("hideout");
    let market = w.building_of_kind(BuildingKind::Market).expect("market");
    let homes: Vec<EntityId> = w.buildings_by_kind[&BuildingKind::Home].iter().copied().take(4).collect();
    w.comp_mut::<Gang>(gang).expect("g").territory = homes.clone();
    {
        let l = w.law_mut().expect("law");
        l.posture = Posture::Crackdown;
        l.target = Some(gang);
    }
    let route = law::new_patrol_route(&mut w);
    assert_eq!(route.len(), 5);
    assert_eq!(route[0], market);
    assert_eq!(route[3], hideout, "the loop passes the Hideout");
    for &stop in [route[1], route[2], route[4]].iter() {
        assert!(homes.contains(&stop), "every Home on the loop is the target's");
    }
    // Under Patrol the v1 loop is back: Market, Bar, Hall, two Homes.
    w.law_mut().expect("law").posture = Posture::Patrol;
    let route = law::new_patrol_route(&mut w);
    assert_eq!(route[1], w.building_of_kind(BuildingKind::Bar).expect("bar"));
    assert_eq!(route[2], w.building_of_kind(BuildingKind::Hall).expect("hall"));
    // A gang with little turf is patrolled around its Hideout.
    w.comp_mut::<Gang>(gang).expect("g").territory.clear();
    let turf = law::crackdown_turf(&w, gang);
    assert_eq!(turf.len(), 6);
    let door = w.comp::<Building>(hideout).expect("b").door;
    let far = w.buildings_by_kind[&BuildingKind::Home]
        .iter()
        .filter_map(|&h| w.comp::<Building>(h).map(|b| b.door.manhattan(door)))
        .max()
        .expect("a Home");
    for h in turf {
        assert!(w.comp::<Building>(h).expect("b").door.manhattan(door) < far);
    }
}

#[test]
fn test_jailbreak_shocks_the_law_into_garrison() {
    let mut w = world(64);
    w.tick = 700;
    law_brain::recompute_captain(&mut w);
    assert_eq!(w.law().map(|l| l.posture), Some(Posture::Patrol));
    let gang = w.gangs()[0];
    law::on_breakout(&mut w, gang, true, 2);
    assert!(w.law().is_some_and(|l| l.shocks.contains(&LawShock::Jailbreak)));
    law_brain::run(&mut w);
    let l = w.law().expect("law");
    assert_eq!(l.posture, Posture::Garrison);
    assert!(l.shocks.is_empty(), "shocks are consumed");
    assert_eq!(l.posture_trace.first().map(|s| s.posture), Some(Posture::Garrison));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Posture && e.text.contains("Patrol -> Garrison")));
    // A week on, the Jail is let go.
    w.tick += (w.config.law.garrison_days + 1) * TICKS_PER_DAY;
    w.tick -= w.tick % TICKS_PER_DAY;
    law_brain::run(&mut w);
    assert_eq!(w.law().map(|l| l.posture), Some(Posture::Patrol));
}

#[test]
fn test_player_pins_and_releases_the_posture() {
    let mut w = world(65);
    w.push_command(PlayerCommand::SetLawPosture(Some(Posture::Garrison)));
    w.run_ticks(1);
    let l = w.law().expect("law");
    assert_eq!(l.posture, Posture::Garrison);
    assert_eq!(l.pinned, Some(Posture::Garrison));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Posture && e.text.contains("pinned")));
    // A daily rescoring does not move a pinned posture.
    w.run_ticks(TICKS_PER_DAY);
    assert_eq!(w.law().map(|l| l.posture), Some(Posture::Garrison));
    w.push_command(PlayerCommand::SetLawPosture(None));
    w.run_ticks(1);
    let l = w.law().expect("law");
    assert_eq!(l.pinned, None);
    assert_eq!(l.posture, Posture::Patrol, "the captain takes over");
    // A pinned Crackdown with nobody to crack down on patrols as usual.
    w.push_command(PlayerCommand::SetLawPosture(Some(Posture::Crackdown)));
    w.run_ticks(1);
    let l = w.law().expect("law");
    assert_eq!(l.posture, Posture::Crackdown);
    assert_eq!(l.target, None);
    let gs = guards(&w);
    let key = 3i64;
    for &g in &gs {
        assert_eq!(law::jail_duty(&w, g, key), law::jail_day(g, key));
    }
}
