//! The M17 `/code-review high` findings (33fa3da..a8880e9), one test each:
//! a won convoy is no State sanction; a death abroad is quiet (no Murder,
//! no heat, no litter, no witness) yet still a gang's succession; a crowned
//! leader keeps the crown through a night in the cells; every spend a
//! parent makes is booked; a data centre is no netrun; a new branch's
//! numeral counts on; a post that fails gives its funding back.

use citysim::components::{Corp, GangMember, LifeKind, Personality, Skills};
use citysim::outside::OutsideAsset;
use citysim::systems::{abroad, gang, parents};
use citysim::{Config, EntityId, EventKind, Gang, PlayerCommand, World, TICKS_PER_DAY};

fn account(w: &World, name: &str) -> u32 {
    w.outside.factions.iter().find(|f| f.name == name).map(|f| f.id).unwrap_or_else(|| panic!("no account {name}"))
}

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.name == name)).expect("corp")
}

fn city() -> World {
    let mut w = World::new(42, Config::load());
    w.run_ticks(4 * TICKS_PER_DAY + 30);
    w
}

fn god(w: &mut World, cmd: PlayerCommand) {
    w.push_command(cmd);
    w.apply_commands();
}

fn free(w: &mut World, n: usize, fighting: f32) -> Vec<EntityId> {
    let c: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&id| {
            citysim::systems::contracts::free_adult(w, id)
                && !w.has::<citysim::Job>(id)
                && !w.has::<GangMember>(id)
                && !citysim::systems::contracts::holds_body(w, id)
        })
        .take(n)
        .collect();
    for &a in &c {
        w.comp_mut::<Skills>(a).expect("skills").fighting = fighting;
        w.comp_mut::<Personality>(a).expect("p").courage = fighting;
    }
    c
}

/// Settle every mission now (the settling alone).
fn settle(w: &mut World) {
    let now = w.tick;
    for m in &mut w.outside.abroad {
        m.back_at = now;
    }
    abroad::daily(w);
}

/// Review 1: a won convoy's sanction is the crew's, not the State's.
#[test]
fn test_a_won_convoy_is_no_state_sanction() {
    let mut w = city();
    w.config.outside.abroad.death_p = 0.0;
    let on = account(&w, "Arasaka Global");
    let crew = free(&mut w, 6, 1.0);
    god(&mut w, PlayerCommand::PostAbroad { buyer: None, on, asset: OutsideAsset::Convoy, crew });
    settle(&mut w);
    let ev = w.events.iter().find(|e| e.kind == EventKind::Sanctioned).expect("the convoy's sanction");
    assert!(!ev.text.contains("State"), "{}", ev.text);
    assert!(parents::sanction_cut(&w, on) > 0.0, "the cut still bites");
}

/// Review 2: a death abroad is no Murder, no violent death in the city's
/// count, no litter and no killing in the word; a gang leader's death
/// abroad still opens the succession.
#[test]
fn test_a_death_abroad_is_quiet_but_opens_the_succession() {
    let mut w = city();
    w.config.outside.abroad.death_p = 1.0;
    let on = account(&w, "Arasaka Global");
    let g = w.gangs().into_iter().max_by_key(|&g| gang::fit_headcount(&w, g)).expect("a gang");
    for a in free(&mut w, 4, 0.5) {
        gang::enlist(&mut w, a, g);
    }
    let leader = w.comp::<Gang>(g).and_then(|x| x.leader).expect("a leader");
    w.vacate_job(leader);
    let mut crew = vec![leader];
    crew.extend(free(&mut w, 3, 0.0));
    for &a in &crew {
        w.comp_mut::<Skills>(a).expect("skills").fighting = 0.0;
    }
    god(&mut w, PlayerCommand::PostAbroad { buyer: None, on, asset: OutsideAsset::Depot, crew: crew.clone() });
    assert!(abroad::is_away(&w, leader), "the leader went");
    let violent0 = w.stats.current.deaths_violence;
    settle(&mut w);
    let dead: Vec<EntityId> = crew.iter().copied().filter(|&a| !citysim::systems::law::living(&w, a)).collect();
    assert!(dead.contains(&leader), "a hopeless job: the leader died abroad");
    assert_eq!(w.stats.current.deaths_violence, violent0, "no violent death in the city's count");
    for &a in &dead {
        let life = w.life.get(a.index as usize).and_then(|l| l.as_ref()).expect("a life");
        assert!(!life.events.iter().any(|e| e.kind == LifeKind::Killed), "no killing on the record");
        assert!(
            w.comp::<citysim::Corpse>(a).is_some_and(|c| c.cause != citysim::DeathCause::Violence),
            "no violent cause"
        );
    }
    w.run_ticks(1);
    assert!(w.events.iter().any(|e| e.kind == EventKind::Succession || e.kind == EventKind::CivilWar), "succession");
}

/// Review 4: the crowned keeps the crown through a night in the cells.
#[test]
fn test_the_crown_outlasts_a_night_in_the_cells() {
    let mut w = city();
    let g = w.gangs().into_iter().max_by_key(|&g| gang::fit_headcount(&w, g)).expect("a gang");
    for a in free(&mut w, 4, 0.5) {
        gang::enlist(&mut w, a, g);
    }
    let ranking = gang::leader_ranking(&w, g);
    let (king, rival) = (ranking[1], ranking[0]);
    w.comp_mut::<Gang>(g).expect("gang").crowned = Some(king);
    gang::recompute_leader(&mut w, g);
    assert_eq!(w.comp::<Gang>(g).and_then(|x| x.leader), Some(king), "the crown over the ranking");
    let jail = w.building_of_kind(citysim::BuildingKind::Jail).expect("a Jail");
    let until = w.tick + 600;
    citysim::systems::law::sentence(&mut w, king, citysim::Crime::Vagrancy, until, jail);
    assert_eq!(w.comp::<Gang>(g).and_then(|x| x.crowned), Some(king), "still crowned inside");
    w.remove::<citysim::Sentence>(king);
    gang::on_member_released(&mut w, king);
    assert_eq!(w.comp::<Gang>(g).and_then(|x| x.leader), Some(king), "back in the chair, not {rival:?}");
}

/// Review 5: what a Reinforce costs abroad and funds in the city is booked
/// as the parent's spend.
#[test]
fn test_every_spend_is_booked() {
    let mut w = city();
    let on = account(&w, "Arasaka Global");
    let spent = |w: &World| w.outside.faction(on).and_then(|f| f.parent.as_ref()).map_or(0, |p| p.ledger.spent_today);
    let s0 = spent(&w);
    parents::force_reinforce(&mut w, on, 1).expect("one body");
    let cfg = w.config.outside.parents.clone();
    assert_eq!(spent(&w) - s0, cfg.body_cost + cfg.body_fund, "the body abroad and its wages in the branch");
}

/// Review 6: a won data centre is no netrun (no UplinkBreached, no run
/// counted); a payload with nowhere to go cuts no edge.
#[test]
fn test_a_data_centre_is_no_netrun() {
    let mut w = city();
    w.config.outside.abroad.death_p = 0.0;
    let on = account(&w, "Arasaka Global");
    let crew = free(&mut w, 6, 1.0);
    let edge0 = w.outside.faction(on).and_then(|f| f.parent.as_ref()).map_or(0.0, |p| p.edge);
    god(&mut w, PlayerCommand::PostAbroad { buyer: None, on, asset: OutsideAsset::DataCentre, crew });
    settle(&mut w);
    assert!(!w.events.iter().any(|e| e.kind == EventKind::UplinkBreached), "no fake netrun");
    assert_eq!(w.outside.today.uplink_runs, 0);
    assert_eq!(w.outside.today.uplink_breached, 0);
    let edge = w.outside.faction(on).and_then(|f| f.parent.as_ref()).map_or(0.0, |p| p.edge);
    assert_eq!(edge, edge0, "no buyer's Lab: the Data stayed abroad, the edge too");
}

/// Review 10: a branch founded after a dead one is "II", the next "III",
/// whatever lives under the brand.
#[test]
fn test_founded_branches_count_on() {
    let mut w = city();
    let parent = account(&w, "Militech International");
    let militech = corp_named(&w, "Militech");
    citysim::systems::corps::bankrupt(&mut w, militech);
    let first = parents::found_branch(&mut w, parent).expect("a branch");
    assert_eq!(w.comp::<Corp>(first).map(|c| c.name.clone()), Some("Militech II".into()));
    citysim::systems::corps::bankrupt(&mut w, first);
    let second = parents::found_branch(&mut w, parent).expect("another");
    assert_eq!(w.comp::<Corp>(second).map(|c| c.name.clone()), Some("Militech III".into()));
}

/// Review extra: a forced Retake whose post fails gives the funding back.
#[test]
fn test_a_failed_retake_post_gives_the_funding_back() {
    let mut w = city();
    let parent = account(&w, "Arasaka Global");
    let branch = corp_named(&w, "Arasaka");
    parents::note_losses(&mut w, parent);
    let b = w.comp::<Corp>(branch).expect("corp").buildings[0];
    let g = w.gangs()[0];
    citysim::systems::corps::move_building(&mut w, b, Some(g));
    parents::note_losses(&mut w, parent);
    for _ in 0..w.config.outside.parents.refusals_to_force {
        parents::retake_step(&mut w, parent);
    }
    // The board is full: the post fails.
    w.config.contracts.max_open = 0;
    let (branch0, parent0) = (w.purse(Some(branch)), w.outside.faction(parent).expect("p").treasury);
    parents::retake_step(&mut w, parent);
    assert!(!w.contracts.values().any(|c| c.kind == citysim::contract::ContractKind::Retake), "no Retake posted");
    assert_eq!(w.purse(Some(branch)), branch0, "the branch keeps nothing it was sent for it");
    assert_eq!(w.outside.faction(parent).expect("p").treasury, parent0, "the parent has its coins back");
}
