//! M16b phase 4 (docs/M16_CONTRACTS.md § 6, § 8): boards, votes and the
//! campaign, and the 16b god commands, one test per mechanism in the
//! shipped seed-42 city: the boards seated on their roster rows, the
//! board's mean Personality, a pinned member's `VoteSwung`, a chair voted
//! out (the new exec), the vote at once on the chair's death, the
//! campaign's swing target and its step, a `Vote` record's Intimidate,
//! `SetPrisonPermit(false)` admitting no new captive, a `Release` hostage
//! demand against a weak and a lawful captain, and the god `Wound`,
//! `Subscribe`, `TagAgent`, `Coerce`, `CallVote` and `ExposeSecret`. Every
//! board, vote and hold here is game state between fictional agents: a
//! record on a struct and a seeded dice roll. With `[governance] enabled = false`
//! the mechanism tests fail (the switch gates every entry point).

use citysim::contract::{
    CoercionStatus, CoercionWhy, ContractKind, ContractStatus, Demand, Leverage, Origin, Posting, Target,
};
use citysim::systems::{captives, contracts, demography, governance, law, leverage};
use citysim::{
    Brain, BuildingKind, Config, Corp, Crime, DeathCause, EntityId, EventKind, Gang, GangMember, Governance,
    Personality, PlayerCommand, Skills, World, TICKS_PER_DAY,
};

fn world() -> World {
    World::new(42, Config::load())
}

/// The seed-42 city with its boards seated (the first midnight's pass).
fn boards() -> World {
    let mut w = world();
    governance::run(&mut w);
    w
}

fn habitat(w: &World) -> EntityId {
    w.corps_of_template("Habitat").first().copied().expect("a Habitat")
}

fn members(w: &World, corp: EntityId) -> (Vec<EntityId>, EntityId) {
    let (m, chair) = governance::board_of(w, corp).expect("a board");
    (m, chair.expect("a chair"))
}

fn has_event(w: &World, kind: EventKind, who: EntityId) -> bool {
    w.events.iter().any(|e| e.kind == kind && e.actors.contains(&who))
}

/// The seed-42 city once two gangs are led and one holds a district.
fn gang_world() -> World {
    static SAVE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let text = SAVE.get_or_init(|| {
        let mut w = world();
        for _ in 0..40 {
            w.run_ticks(TICKS_PER_DAY);
            let led = w.gangs().into_iter().filter(|&g| w.comp::<Gang>(g).is_some_and(|x| x.leader.is_some())).count();
            let held = w.gangs().into_iter().any(|g| !citysim::systems::gang::held_districts(&w, g).is_empty());
            if led >= 2 && held {
                break;
            }
        }
        citysim::save::to_ron(&w)
    });
    citysim::save::from_ron(text).expect("loads")
}

/// A led gang with members.
fn led_gang(w: &World) -> EntityId {
    w.gangs()
        .into_iter()
        .find(|&g| w.comp::<Gang>(g).is_some_and(|x| x.leader.is_some() && x.members.len() >= 2))
        .expect("a led gang")
}

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| law::living(w, id) && demography::is_adult(w, id)).collect()
}

fn strangers(w: &World, n: usize) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = Vec::new();
    for a in adults(w) {
        if w.has::<GangMember>(a) || law::is_guard(w, a) || w.has::<citysim::Sentence>(a) {
            continue;
        }
        if w.spouse_of(a).is_some() || out.iter().any(|&o| w.edge(o, a).is_some()) {
            continue;
        }
        if w.comp::<Brain>(a).is_none_or(|b| b.cuffed_by.is_some()) {
            continue;
        }
        out.push(a);
        if out.len() == n {
            return out;
        }
    }
    panic!("{n} strangers");
}

fn heavy(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.lawfulness = 0.0;
        p.courage = 1.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.fighting = 1.0;
        s.intimidation = 1.0;
        s.persuasion = 1.0;
    }
}

fn soft(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.courage = 0.0;
        p.greed = 1.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.fighting = 0.0;
        s.knowledge = 0.0;
        s.intimidation = 0.0;
    }
}

fn until_ok(w: &mut World, mut f: impl FnMut(&mut World) -> bool) -> bool {
    for _ in 0..300 {
        w.lev_tried.clear();
        if f(w) {
            return true;
        }
        w.tick += 1;
    }
    false
}

/// Spec § 6: Habitat and Nutrix (every copy) seat a board at the first
/// midnight, the exec as chair plus the employees of the highest standing;
/// every other corp stays a dictatorship.
#[test]
fn test_boards_seated_on_their_rows() {
    let w = boards();
    for c in w.corps() {
        let row = w.corp_template(c).to_string();
        let board = governance::board_of(&w, c);
        if row == "Habitat" || row == "Nutrix" {
            let (m, chair) = board.unwrap_or_else(|| panic!("{row} has a board"));
            let exec = w.comp::<Corp>(c).and_then(|x| x.exec);
            assert_eq!(chair, exec, "{row}: the exec chairs");
            assert!(m.contains(&exec.unwrap()));
            assert!(m.len() > 1 && m.len() <= w.config.governance.board_size, "{row}: {} members", m.len());
        } else {
            assert!(board.is_none(), "{row} is a dictatorship");
        }
    }
    assert!(w.stats.current.gov.boards >= 2);
}

/// Spec § 6: a board corp's brain reads the members' mean Personality,
/// the chair counted twice; off, the exec's.
#[test]
fn test_board_personality_is_the_weighted_mean() {
    let mut w = boards();
    let c = habitat(&w);
    let (m, chair) = members(&w, c);
    let mut sum = 0.0;
    let mut n = 0.0;
    for &x in &m {
        let k = if x == chair { 2.0 } else { 1.0 };
        sum += k * w.comp::<Personality>(x).unwrap().greed;
        n += k;
    }
    let p = governance::personality(&w, c).unwrap();
    assert!((p.greed - sum / n).abs() < 1e-5, "{} vs {}", p.greed, sum / n);
    w.config.governance.enabled = false;
    let p = governance::personality(&w, c).unwrap();
    assert_eq!(p.greed, w.comp::<Personality>(chair).unwrap().greed);
}

/// Spec § 11: a vote under a pinned member records `VoteSwung`; the
/// ballots are kept and the pin's coercion is complied.
#[test]
fn test_pinned_member_swings_the_vote() {
    let mut w = boards();
    w.config.governance.ambition_min = 0.0;
    let c = habitat(&w);
    let (m, chair) = members(&w, c);
    let others: Vec<EntityId> = m.iter().copied().filter(|&x| x != chair).collect();
    let (swing, challenger) = (others[0], others[1]);
    let id = leverage::force(
        &mut w,
        Some(challenger),
        challenger,
        swing,
        Leverage::Threat,
        Demand::Vote { corp: c, for_: challenger },
        CoercionWhy::Vote,
        None,
        None,
    );
    assert_eq!(governance::pinned_for(&w, c, swing), Some(challenger));
    governance::vote(&mut w, c, "a test");
    assert!(has_event(&w, EventKind::VoteSwung, swing), "the pinned member swung");
    assert!(has_event(&w, EventKind::VoteHeld, c));
    assert_eq!(w.stats.current.gov.votes_swung, 1);
    assert_eq!(w.coercions[&id].status, CoercionStatus::Complied);
    let Governance::Board { votes, .. } = &w.comp::<Corp>(c).unwrap().governance else { panic!("a board") };
    assert!(votes.contains(&(swing, challenger)));
}

/// Spec § 6: a challenger with the majority takes the chair and becomes
/// the exec (`chairs_fallen`); ties go to the chair.
#[test]
fn test_chair_voted_out_is_the_new_exec() {
    let mut w = boards();
    w.config.governance.ambition_min = 0.0;
    let c = habitat(&w);
    let (m, chair) = members(&w, c);
    let challenger = m.iter().copied().find(|&x| x != chair).unwrap();
    for &x in m.iter().filter(|&&x| x != challenger) {
        governance::pin(&mut w, c, x, challenger);
    }
    assert_eq!(governance::vote(&mut w, c, "a test"), Some(challenger));
    assert_eq!(w.comp::<Corp>(c).unwrap().exec, Some(challenger));
    assert_eq!(governance::board_of(&w, c).unwrap().1, Some(challenger));
    assert_eq!(w.stats.current.gov.chairs_fallen, 1);
    // A lone pin for each of two members: one vote apiece, the chair keeps it.
    let mut w = boards();
    w.config.governance.ambition_min = 0.0;
    let (m, chair) = members(&w, c);
    let others: Vec<EntityId> = m.iter().copied().filter(|&x| x != chair).collect();
    for &x in &others {
        governance::pin(&mut w, c, x, x);
    }
    assert_eq!(governance::vote(&mut w, c, "a test"), Some(chair), "every tie goes to the chair");
}

/// Spec § 6: the chair's death calls the vote at once (the next hourly pass).
#[test]
fn test_chair_death_votes_at_once() {
    let mut w = boards();
    let c = habitat(&w);
    let (_, chair) = members(&w, c);
    w.kill_by(chair, DeathCause::Violence, None);
    w.tick += 60;
    governance::run(&mut w);
    assert!(has_event(&w, EventKind::VoteHeld, c), "a vote on the chair's death");
    let (_, new) = members(&w, c);
    assert_ne!(new, chair);
    assert!(law::living(&w, new));
    assert_eq!(w.comp::<Corp>(c).unwrap().exec, Some(new));
}

/// Spec § 6: within `campaign_days` the challenger losing by one or two
/// leans on the swing member of the closest margin, one a day (a tried
/// mark: not the same member twice in the week).
#[test]
fn test_campaign_targets_the_swing_member() {
    let mut w = boards();
    w.config.governance.ambition_min = 0.0;
    let c = habitat(&w);
    let (m, chair) = members(&w, c);
    let others: Vec<EntityId> = m.iter().copied().filter(|&x| x != chair).collect();
    // Everyone votes for themself, one member for the chair: 2 against 1.
    for &x in &others {
        governance::pin(&mut w, c, x, x);
    }
    governance::pin(&mut w, c, others[0], chair);
    let (ch, swing) = governance::campaign_target(&w, c).expect("a campaign");
    assert_ne!(ch, chair);
    assert_ne!(swing, ch);
    assert!(governance::campaign(&mut w, c));
    assert_eq!(w.stats.current.gov.campaign_moves, 1);
    assert!(leverage::tried_recently(&w, ch, swing, leverage::tried::VOTE));
}

/// Spec § 6, § 1 (the Board row): a lawful challenger who cannot talk
/// buys a `Persuade` (`Origin::Board`) on the swing member; the taker's
/// Charm, when it lands, pins the member's vote for the buyer.
#[test]
fn test_campaign_buys_a_persuade_for_a_weak_talker() {
    let mut w = boards();
    w.config.governance.ambition_min = 0.0;
    let c = habitat(&w);
    let (m, chair) = members(&w, c);
    let others: Vec<EntityId> = m.iter().copied().filter(|&x| x != chair).collect();
    for &x in &others {
        governance::pin(&mut w, c, x, x);
    }
    governance::pin(&mut w, c, others[0], chair);
    let (ch, swing) = governance::campaign_target(&w, c).expect("a campaign");
    if let Some(p) = w.comp_mut::<Personality>(ch) {
        p.lawfulness = 0.9;
    }
    if let Some(s) = w.comp_mut::<Skills>(ch) {
        s.persuasion = 0.0;
    }
    if let Some(x) = w.comp_mut::<citysim::Wallet>(ch) {
        x.coins = 10_000;
    }
    assert!(governance::campaign(&mut w, c));
    assert_eq!(w.stats.current.gov.board_persuades, 1);
    let id = *w
        .contracts
        .iter()
        .find(|(_, x)| x.kind == ContractKind::Persuade && x.origin == Origin::Board)
        .expect("a board Persuade")
        .0;
    assert_eq!(w.contracts[&id].target, Target::Agent(swing));
    let [taker] = strangers(&w, 1)[..] else { unreachable!() };
    heavy(&mut w, taker);
    w.config.contracts.max_attempts = 200;
    let done = until_ok(&mut w, |w| {
        if w.contracts[&id].status == ContractStatus::Open {
            contracts::god_take(w, id, taker).expect("taken");
        }
        contracts::resolve_ledger(w, id);
        w.contracts[&id].status == ContractStatus::Fulfilled
    });
    assert!(done, "the Charm landed");
    assert_eq!(governance::pinned_for(&w, c, swing), Some(ch));
}

/// Spec § 6, § 1: a `Vote` record's taker Intimidates the member; a win
/// opens the `Vote` coercion for the buyer (the pin) and fulfils it.
#[test]
fn test_vote_record_pins_the_member() {
    let mut w = boards();
    let c = habitat(&w);
    let (m, chair) = members(&w, c);
    let others: Vec<EntityId> = m.iter().copied().filter(|&x| x != chair).collect();
    let (buyer, member) = (others[0], others[1]);
    soft(&mut w, member);
    if let Some(x) = w.comp_mut::<citysim::Wallet>(buyer) {
        x.coins = 10_000;
    }
    let [taker] = strangers(&w, 1)[..] else { unreachable!() };
    heavy(&mut w, taker);
    let p = Posting {
        buyer: Some(buyer),
        agent: Some(buyer),
        kind: ContractKind::Vote,
        target: Target::Agent(member),
        broker: None,
        deadline_days: 7,
        origin: Origin::Board,
        price: None,
    };
    let id = contracts::post(&mut w, p).expect("posted");
    w.config.contracts.max_attempts = 200;
    let ok = until_ok(&mut w, |w| {
        if w.contracts[&id].status == ContractStatus::Open {
            contracts::god_take(w, id, taker).expect("taken");
        }
        contracts::resolve_ledger(w, id);
        w.contracts[&id].status == ContractStatus::Fulfilled
    });
    assert!(ok, "the member was leaned on within the tries");
    assert_eq!(governance::pinned_for(&w, c, member), Some(buyer));
}

/// Spec § 8, § 11: `SetPrisonPermit(false)` builds no Prison and admits no
/// new captive to a standing one.
#[test]
fn test_prison_permit_off_admits_no_captive() {
    let mut w = world();
    let corp = w.corps()[0];
    w.comp_mut::<Corp>(corp).unwrap().treasury = 100_000;
    let prison = captives::build_prison(&mut w, corp).expect("a Prison with the permit");
    w.push_command(PlayerCommand::SetPrisonPermit(false));
    w.apply_commands();
    assert!(!w.levers.prison_permit);
    assert!(captives::build_prison(&mut w, corp).is_none(), "no new Prison");
    let [v] = strangers(&w, 1)[..] else { unreachable!() };
    let exec = w.comp::<Corp>(corp).and_then(|c| c.exec).unwrap();
    assert!(!captives::take(&mut w, v, Some(corp), exec, prison, None, None), "no new captive");
    w.push_command(PlayerCommand::SetPrisonPermit(true));
    w.apply_commands();
    assert!(captives::take(&mut w, v, Some(corp), exec, prison, None, None));
}

/// Spec § 8, § 11 (the captain's spouse as a hostage): a `Release` demand
/// over the captain lets the convict out only while the law is weak and
/// the captain bent (`competence < 0.4`, lawfulness < 0.5).
#[test]
fn test_release_demand_and_the_captain() {
    for (bent, freed) in [(true, true), (false, false)] {
        let mut w = gang_world();
        let g = led_gang(&w);
        let h = w.hideout_of(g).expect("a Hideout");
        let leader = w.comp::<Gang>(g).and_then(|x| x.leader).expect("a leader");
        heavy(&mut w, leader);
        citysim::systems::law_brain::recompute_captain(&mut w);
        let captain = w.law().and_then(|l| l.captain).expect("a captain");
        let [v, convict] = strangers(&w, 2)[..] else { unreachable!() };
        let jail = w.building_of_kind(BuildingKind::Jail).unwrap();
        let until = w.tick + 30 * TICKS_PER_DAY;
        law::sentence(&mut w, convict, Crime::Assault, until, jail);
        if let Some(l) = w.law_mut() {
            l.competence = if bent { 0.1 } else { 0.9 };
        }
        if let Some(p) = w.comp_mut::<Personality>(captain) {
            p.lawfulness = if bent { 0.1 } else { 0.9 };
            p.courage = 0.0;
        }
        w.push_command(PlayerCommand::Hostage {
            captive: v,
            holder: Some(g),
            at: h,
            demand: Some(Demand::Release(convict)),
        });
        w.apply_commands();
        assert!(captives::is_captive(&w, v), "held");
        let cid = w.captives[&v].coercion.expect("a coercion");
        assert_eq!(w.coercions[&cid].over, captain);
        let out = until_ok(&mut w, |w| captives::roll_hostage(w, v));
        assert_eq!(out, freed, "bent {bent}");
        assert_eq!(!w.has::<citysim::Sentence>(convict), freed, "bent {bent}");
    }
}

/// Spec § 8: the god commands of 16b, each applied through the queue.
#[test]
fn test_god_commands_apply() {
    let mut w = boards();
    let c = habitat(&w);
    let [a, b] = strangers(&w, 2)[..] else { unreachable!() };
    let g = w.gangs()[0];
    let sec = w
        .corps()
        .into_iter()
        .find(|&x| w.comp::<Corp>(x).is_some_and(|cc| cc.niches.contains(&citysim::Niche::Security)))
        .expect("a Security corp");
    if let Some(x) = w.comp_mut::<citysim::Wallet>(a) {
        x.coins = 10_000;
    }
    w.push_command(PlayerCommand::Wound { agent: b, by: None });
    w.push_command(PlayerCommand::Subscribe { agent: a, corp: sec });
    w.push_command(PlayerCommand::TagAgent { agent: a, owner: g, tier: 2 });
    w.push_command(PlayerCommand::Coerce {
        agent: a,
        over: members(&w, c).0[1],
        leverage: Leverage::Threat,
        demand: Demand::Vote { corp: c, for_: a },
    });
    w.push_command(PlayerCommand::CallVote(c));
    w.apply_commands();
    assert!(citysim::systems::wounded::is_down(&w, b), "Wound");
    assert_eq!(w.trauma_clients.get(&a), Some(&sec), "Subscribe");
    assert!(w.tags.iter().any(|t| t.on == a && t.owner == g && t.tier == 2), "TagAgent");
    assert!(w.coercions.values().any(|x| x.agent == a && x.why == CoercionWhy::Vote), "Coerce");
    assert!(has_event(&w, EventKind::VoteHeld, c), "CallVote");
    assert!(!w.events.iter().any(|e| e.kind == EventKind::PlayerActionFailed), "no failure");
}

/// Spec § 8: `ExposeSecret` posts a Secret coercion's deed at reach 1.0;
/// the next daily pass lapses the blackmail.
#[test]
fn test_expose_secret_lapses_the_blackmail() {
    let mut w = world();
    let [holder, over, victim] = strangers(&w, 3)[..] else { unreachable!() };
    if let Some(x) = w.comp_mut::<citysim::Wallet>(over) {
        x.coins = 10_000;
    }
    let lev = Leverage::Secret { deed: citysim::word::Deed::Assaulted, actor: over, object: Some(victim), day: 0 };
    let id = leverage::force(
        &mut w,
        Some(holder),
        holder,
        over,
        lev,
        Demand::Coins { amount: 50, weekly: true },
        CoercionWhy::Blackmail,
        None,
        None,
    );
    w.push_command(PlayerCommand::ExposeSecret(id));
    w.apply_commands();
    assert!(leverage::reach(&w, citysim::word::Deed::Assaulted, over, Some(victim)) >= 0.99);
    w.run_ticks(TICKS_PER_DAY);
    assert_eq!(w.coercions[&id].status, CoercionStatus::Lapsed);
}

/// M16b review finding 3: a `Vote` coercion whose corp lost its board
/// breaks at the next midnight (it never held a slot forever).
#[test]
fn test_vote_coercion_breaks_with_the_board() {
    let mut w = boards();
    let c = habitat(&w);
    let (m, chair) = members(&w, c);
    let others: Vec<EntityId> = m.iter().copied().filter(|&x| x != chair).collect();
    let id = leverage::force(
        &mut w,
        Some(others[0]),
        others[0],
        others[1],
        Leverage::Threat,
        Demand::Vote { corp: c, for_: others[0] },
        CoercionWhy::Vote,
        None,
        None,
    );
    w.comp_mut::<Corp>(c).unwrap().governance = Governance::Dictator;
    w.run_ticks(TICKS_PER_DAY);
    assert_eq!(w.coercions.get(&id).map(|x| x.status), Some(CoercionStatus::Broken));
}
