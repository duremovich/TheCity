//! M17 phase 3 (docs/M17_OUTSIDE.md, "Revised 2026-10-12" and § 2, § 4):
//! bodies. A parent's Reinforce sends Outsiders (an agent from abroad with a
//! job at the branch and a tour), a lost building is bought back or taken
//! back by a `Retake`, a dead branch is refounded, a parent can abandon its
//! branches and a rich local corp can found a parent; a syndicate's brothers
//! come after a decapitation (late and capped) and an emptied gang re-forms
//! only through its syndicate; the State funds guards, mandates a harder
//! posture and withdraws. Every crossing keeps the coin identity.

use citysim::contract::{ContractKind, Origin, Target};
use citysim::outside::{OutsideKind, OutsideOrder};
use citysim::systems::{econ, gang, parents};
use citysim::{Config, Corp, EntityId, EventKind, Gang, Job, Personality, World, TICKS_PER_DAY};

fn account(w: &World, name: &str) -> citysim::outside::OutsideFaction {
    w.outside.factions.iter().find(|f| f.name == name).cloned().unwrap_or_else(|| panic!("no account {name}"))
}

fn corp_named(w: &World, name: &str) -> EntityId {
    w.corps().into_iter().find(|&c| w.comp::<Corp>(c).is_some_and(|x| x.name == name)).expect("corp")
}

/// The seed-42 city on day 4, past the orders' minimum hold.
fn city() -> World {
    let mut w = World::new(42, Config::load());
    w.run_ticks(4 * TICKS_PER_DAY + 30);
    w
}

fn outsiders_of(w: &World, id: u32) -> Vec<EntityId> {
    w.outside.outsiders.iter().filter(|(_, o)| o.from == id && !o.brother).map(|(&a, _)| a).collect()
}

#[test]
fn test_remit_gap_reads_sustained_losses_only() {
    let mut w = city();
    let parent = account(&w, "Militech International").id;
    let militech = corp_named(&w, "Militech");
    w.comp_mut::<Corp>(militech).expect("corp").cashflow = [500, -20, 300, -40, 600, 10, 250].into_iter().collect();
    assert_eq!(parents::remit_gap(&w, parent), 0.0, "a profitable week with two bad days is no loss");
    w.comp_mut::<Corp>(militech).expect("corp").cashflow = [-500, -20, 30, -40, -600, -10, -250].into_iter().collect();
    assert_eq!(parents::remit_gap(&w, parent), 1.0, "a week in the red is");
    assert_eq!(parents::branch_loss(1000, 1000, 0.0), 0.0);
    assert!((parents::branch_loss(500, 1000, 1.0) - 0.7).abs() < 1e-5);
}

/// Reinforce: at most `bodies_per_day` a day and `bodies_per_call` an
/// episode, each an Outsider with a job at a branch building and the
/// episode's cohort; the body cost is paid abroad (minted out).
#[test]
fn test_reinforce_caps_jobs_and_cohort() {
    let mut w = city();
    let parent = account(&w, "Arasaka Global").id;
    let cfg = w.config.outside.parents.clone();
    let branch = corp_named(&w, "Arasaka");
    let base = econ::identity(&w);
    assert!(parents::start_reinforce(&mut w, parent));
    let mut sent = Vec::new();
    for _ in 0..5 {
        let n = parents::reinforce_step(&mut w, parent);
        assert!(n <= cfg.bodies_per_day);
        sent.push(n);
    }
    let total: u32 = sent.iter().sum();
    assert_eq!(total, cfg.bodies_per_call, "an episode's bodies: {sent:?}");
    let bodies = outsiders_of(&w, parent);
    assert_eq!(bodies.len() as u32, cfg.bodies_per_call);
    let cohort = w.outside.outsiders[&bodies[0]].cohort;
    for &b in &bodies {
        let job = w.comp::<Job>(b).expect("a job on arrival");
        let at = job.employer.expect("an employer");
        assert_eq!(w.owner_of(at), Some(branch), "a job at a branch building");
        assert_eq!(w.outside.outsiders[&b].cohort, cohort);
    }
    assert_eq!(econ::identity(&w), base, "the bodies' costs are booked");
    assert!(w.events.iter().any(|e| e.kind == EventKind::OutsidersArrived));
    // Not again inside the cooldown.
    assert!(!parents::start_reinforce(&mut w, parent), "the cooldown holds");
}

/// A tour ends: an Outsider with no Spouse, child or building leaves; one
/// with a Spouse settles and stays.
#[test]
fn test_outsider_tour_ends_or_settles() {
    let mut w = city();
    let parent = account(&w, "Arasaka Global").id;
    assert!(parents::start_reinforce(&mut w, parent));
    parents::reinforce_step(&mut w, parent);
    let bodies = outsiders_of(&w, parent);
    let [a, b] = bodies[..] else { panic!("two bodies: {bodies:?}") };
    let spouse = w
        .citizens()
        .into_iter()
        .find(|&x| x != a && x != b && !w.spouses.contains_key(&x) && citysim::systems::demography::is_adult(&w, x))
        .expect("an adult");
    w.set_spouse(b, spouse);
    let now = w.tick;
    for x in [a, b] {
        w.outside.outsiders.get_mut(&x).expect("outsider").tour_until = now;
    }
    parents::tours(&mut w);
    assert!(!w.outside.outsiders.contains_key(&a) && !w.outside.outsiders.contains_key(&b));
    assert!(w.comp::<citysim::Brain>(a).is_none_or(|br| br.emigrating), "the lone one leaves");
    assert!(w.comp::<citysim::Brain>(b).is_some_and(|br| !br.emigrating), "the married one stays");
    assert!(w.events.iter().any(|e| e.kind == EventKind::OutsiderLeft));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Settled));
}

/// A lost building: a greedy agent holder sells it back at the premium; a
/// gang never sells, and after `refusals_to_force` refusals a `Retake` is
/// posted with the parent as its origin; a won Retake forces the sale.
#[test]
fn test_buyback_then_retake_forces_the_sale() {
    let mut w = city();
    let parent = account(&w, "Arasaka Global").id;
    let branch = corp_named(&w, "Arasaka");
    parents::note_losses(&mut w, parent);
    let b = w.comp::<Corp>(branch).expect("corp").buildings[0];
    let holder = w.citizens().into_iter().find(|&x| citysim::systems::demography::is_adult(&w, x)).expect("an adult");
    w.comp_mut::<Personality>(holder).expect("p").greed = 0.9;
    citysim::systems::corps::move_building(&mut w, b, Some(holder));
    parents::note_losses(&mut w, parent);
    let base = econ::identity(&w);
    let coins0 = w.purse(Some(holder));
    parents::retake_step(&mut w, parent);
    assert_eq!(w.owner_of(b), Some(branch), "bought back");
    assert!(w.purse(Some(holder)) > coins0, "at a premium");
    assert_eq!(econ::identity(&w), base);
    assert!(w.events.iter().any(|e| e.kind == EventKind::BuyBackOffered));
    // A gang holds it now: it never sells.
    let g = w.gangs()[0];
    citysim::systems::corps::move_building(&mut w, b, Some(g));
    parents::note_losses(&mut w, parent);
    for _ in 0..w.config.outside.parents.refusals_to_force {
        parents::retake_step(&mut w, parent);
        assert_eq!(w.owner_of(b), Some(g), "a gang never sells");
    }
    parents::retake_step(&mut w, parent);
    let (cid, c) = w
        .contracts
        .iter()
        .find(|(_, c)| c.kind == ContractKind::Retake)
        .map(|(&i, c)| (i, c.clone()))
        .expect("a Retake posted");
    assert_eq!(c.origin, Origin::Parent);
    assert_eq!(c.target, Target::Building(b));
    assert_eq!(c.buyer, Some(branch));
    let base = econ::identity(&w);
    assert!(parents::retake_win(&mut w, cid), "the win forces the sale");
    assert_eq!(w.owner_of(b), Some(branch));
    assert_eq!(econ::identity(&w), base);
    assert!(w.events.iter().any(|e| e.kind == EventKind::BranchRetook));
}

/// A parent whose corp branch died founds a new one: a corp with the
/// parent, the seed treasury from abroad, a building.
#[test]
fn test_found_branch_after_the_branch_died() {
    let mut w = city();
    let parent = account(&w, "Militech International").id;
    let militech = corp_named(&w, "Militech");
    citysim::systems::corps::bankrupt(&mut w, militech);
    let base = econ::identity(&w);
    let fresh = parents::found_branch(&mut w, parent).expect("a branch founded");
    let c = w.comp::<Corp>(fresh).expect("corp");
    assert_eq!(c.parent, Some(parent));
    assert!(c.name.starts_with("Militech"));
    assert!(c.treasury > 0, "seeded from abroad");
    assert_eq!(econ::identity(&w), base);
    assert!(account(&w, "Militech International").parent.expect("p").branches.contains(&fresh));
    assert!(w.events.iter().any(|e| e.kind == EventKind::BranchFounded));
}

/// A local corp past the thresholds founds a parent: its coins move abroad
/// as the account's treasury (a crossing, nothing minted).
#[test]
fn test_local_corp_founds_a_parent() {
    let mut w = city();
    let nutrix = corp_named(&w, "Nutrix");
    let cfg = w.config.outside.parents.clone();
    {
        let c = w.comp_mut::<Corp>(nutrix).expect("corp");
        c.treasury = cfg.found_parent_treasury * 2;
        for v in c.share_hist.values_mut() {
            *v = std::iter::repeat_n(0.9, 30).collect();
        }
    }
    w.outside.found_streak.insert(nutrix, cfg.found_parent_days);
    let base = econ::identity(&w);
    let n0 = w.outside.parents().count();
    parents::found_parents(&mut w);
    assert_eq!(w.outside.parents().count(), n0 + 1);
    let f = w.outside.factions.last().expect("account").clone();
    assert_eq!(f.kind, OutsideKind::Megacorp);
    assert_eq!(w.comp::<Corp>(nutrix).and_then(|c| c.parent), Some(f.id));
    assert!(f.treasury > 0);
    assert_eq!(econ::identity(&w), base, "the coins crossed out, nothing minted");
    assert!(w.events.iter().any(|e| e.kind == EventKind::ParentFounded));
}

/// Abandon cuts the branches loose and ends the Outsiders' tours.
#[test]
fn test_abandon_cuts_branches_loose() {
    let mut w = city();
    let parent = account(&w, "Arasaka Global").id;
    let branch = corp_named(&w, "Arasaka");
    assert!(parents::start_reinforce(&mut w, parent));
    parents::reinforce_step(&mut w, parent);
    parents::abandon(&mut w, parent);
    assert_eq!(w.comp::<Corp>(branch).and_then(|c| c.parent), None);
    let p = account(&w, "Arasaka Global").parent.expect("p");
    assert!(p.branches.is_empty() && p.abandoned_until.is_some());
    assert!(w.outside.outsiders.values().filter(|o| o.from == parent).all(|o| o.tour_until <= w.tick));
    assert!(w.events.iter().any(|e| e.kind == EventKind::Abandoned));
}

/// Brothers come `brother_delay_days` after a decapitation, not before,
/// and a second decapitation inside the cooldown sends none.
#[test]
fn test_brothers_after_the_delay_and_not_inside_the_cooldown() {
    let mut w = city();
    let hollow = account(&w, "Hollow Brothers");
    let g = hollow.parent.as_ref().expect("p").branches[0];
    let cfg = w.config.outside.parents.clone();
    let n0 = w.comp::<Gang>(g).expect("gang").members.len();
    assert!(parents::on_decapitation(&mut w, g, None), "brothers scheduled");
    w.run_ticks((u64::from(cfg.brother_delay_days) - 1) * TICKS_PER_DAY);
    assert!(!w.events.iter().any(|e| e.kind == EventKind::BrothersArrived), "not before the delay");
    w.run_ticks(2 * TICKS_PER_DAY);
    assert!(w.events.iter().any(|e| e.kind == EventKind::BrothersArrived));
    let brothers: Vec<EntityId> = w.outside.outsiders.iter().filter(|(_, o)| o.brother).map(|(&a, _)| a).collect();
    assert_eq!(brothers.len() as u32, cfg.brothers);
    assert!(brothers.iter().all(|&b| w.gang_of(b) == Some(g)), "members of the gang");
    assert!(w.comp::<Gang>(g).expect("gang").members.len() >= n0, "the roster grew back");
    assert!(!parents::on_decapitation(&mut w, g, None), "inside the cooldown: none");
}

/// An emptied gang with a syndicate takes no bootstrap recruit; it
/// re-forms after `refound_days` through its syndicate.
#[test]
fn test_refound_only_after_refound_days() {
    let mut w = city();
    let hollow = account(&w, "Hollow Brothers");
    let g = hollow.parent.as_ref().expect("p").branches[0];
    let members = w.comp::<Gang>(g).expect("gang").members.clone();
    for m in members {
        gang::leave(&mut w, m, "test");
    }
    let now = w.tick;
    {
        let x = w.comp_mut::<Gang>(g).expect("gang");
        x.emptied = true;
        x.empty_since = Some(now);
    }
    assert!(!gang::may_reform(&w, g), "no bootstrap with a syndicate");
    let cfg = w.config.outside.parents.clone();
    w.run_ticks((u64::from(cfg.refound_days) - 2) * TICKS_PER_DAY);
    assert!(!w.events.iter().any(|e| e.kind == EventKind::GangRefounded), "not before refound_days");
    w.run_ticks((u64::from(cfg.brother_delay_days) + 4) * TICKS_PER_DAY);
    assert!(w.events.iter().any(|e| e.kind == EventKind::GangRefounded));
    assert!(!w.comp::<Gang>(g).expect("gang").members.is_empty(), "re-formed");
}

/// The State: Fund under high heat and an honest captain, Withdraw on a
/// calm city or a corrupt one; Fund raises the State's guards and pays a
/// week's wages into the Treasury; a Mandate tilts the law.
#[test]
fn test_state_funds_mandates_and_withdraws() {
    let w0 = city();
    let cfg = w0.config.outside.parents.clone();
    let best = |v: Vec<(OutsideOrder, f32)>| v.first().map(|x| x.0);
    assert_eq!(best(parents::state_scores(&cfg, 0.9, 0.1, 0.5, 0, cfg.state_guards_max)), Some(OutsideOrder::Fund));
    assert_eq!(
        best(parents::state_scores(&cfg, 0.05, 0.9, 0.5, 8, cfg.state_guards_max)),
        Some(OutsideOrder::Withdraw)
    );
    assert_eq!(best(parents::state_scores(&cfg, 0.05, 0.1, 0.5, 0, cfg.state_guards_max)), Some(OutsideOrder::Hold));
    let mut w = w0;
    let base = econ::identity(&w);
    let city0 = w.treasury().map_or(0, |t| t.coins);
    parents::state_act(&mut w, OutsideOrder::Fund);
    assert_eq!(w.outside.state_guards, cfg.state_guards_step);
    assert!(w.treasury().map_or(0, |t| t.coins) > city0, "a week's wages granted");
    assert_eq!(econ::identity(&w), base);
    assert!(w.events.iter().any(|e| e.kind == EventKind::StateGrant));
    parents::state_act(&mut w, OutsideOrder::Mandate);
    assert!(parents::mandate_active(&w));
    assert!(w.events.iter().any(|e| e.kind == EventKind::StateMandate));
    parents::state_act(&mut w, OutsideOrder::Withdraw);
    assert_eq!(w.outside.state_guards, 0);
    assert!(w.events.iter().any(|e| e.kind == EventKind::StateWithdrew));
}

/// A save mid-run with Outsiders and the State's guards resumes byte for byte.
#[test]
fn test_save_round_trip_with_bodies() {
    let mut w = city();
    let parent = account(&w, "Arasaka Global").id;
    assert!(parents::start_reinforce(&mut w, parent));
    parents::reinforce_step(&mut w, parent);
    parents::state_act(&mut w, OutsideOrder::Fund);
    w.run_ticks(TICKS_PER_DAY + 333);
    let text = citysim::save::to_ron(&w);
    assert!(text.contains("outsiders"));
    let mut back = citysim::save::from_ron(&text).expect("loads");
    assert_eq!(citysim::save::to_ron(&back), text);
    w.run_ticks(2 * TICKS_PER_DAY);
    back.run_ticks(2 * TICKS_PER_DAY);
    assert_eq!(citysim::save::to_ron(&back), citysim::save::to_ron(&w));
}
