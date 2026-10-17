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

/// Bug 6 (the pitch): an elder opener with a rickshaw-puller's arms vends
/// (3 of 4 street workers were 64-68), at the free spot of his district
/// nearest his home (the first free one meant 47-168 min walks).
#[test]
fn test_bug6_elder_opens_a_vendor_pitch_at_the_nearest_spot() {
    use citysim::systems::streetwork;
    let mut c = Config::load();
    c.jobs2.p_pitch_open = 1.0;
    c.jobs2.pitch_after_days = 0;
    c.jobs2.pitches_per_district = 50;
    let mut w = World::new(42, c);
    // The HangOut spots (the pitches' sites) are laid out in the first day.
    w.run_ticks(citysim::TICKS_PER_DAY);
    let idle = jobless_housed(&w);
    // No other opener (housed or not) holds the stake.
    for a in w.citizens() {
        if w.has::<citysim::Wallet>(a) && !w.has::<Job>(a) {
            set_coins(&mut w, a, 0);
        }
    }
    // One whose home district has spots (two or more free).
    let used: Vec<citysim::TilePos> = w.jobs2.pitches.values().map(|p| p.spot).collect();
    let a = *idle
        .iter()
        .find(|&&x| {
            citysim::systems::contracts::home_door(&w, x).is_some_and(|h| {
                w.spots
                    .get(w.district_of(h).index())
                    .is_some_and(|s| s.iter().filter(|t| !used.contains(&t.tile)).count() >= 2)
            })
        })
        .expect("an opener with spots at home");
    set_coins(&mut w, a, 50);
    w.comp_mut::<citysim::Skills>(a).expect("skills").fighting = 0.95;
    w.comp_mut::<citysim::Skills>(a).expect("skills").knowledge = 0.0;
    let elder = (w.config.ages.frail_from_years + 6) * citysim::time::DAYS_PER_YEAR as u32;
    w.comp_mut::<citysim::Identity>(a).expect("id").age_days = elder;
    let home = citysim::systems::contracts::home_door(&w, a).expect("a home");
    let d = w.district_of(home);
    let nearest = w.spots[d.index()]
        .iter()
        .map(|x| x.tile)
        .filter(|t| !used.contains(t))
        .map(|t| t.manhattan(home))
        .min()
        .expect("a free spot");
    streetwork::daily(&mut w);
    let p = w.jobs2.pitches.get(&a).expect("the elder opened a pitch");
    let kind = w.config.trade(p.trade).and_then(streetwork::pitch_kind);
    assert_eq!(kind, Some(streetwork::PitchKind::Vendor), "an elder vends");
    assert_eq!(p.spot.manhattan(home), nearest, "the free spot nearest home");
}

/// Bug 6 (the hire): a knowledge row hires no one under
/// `hire_knowledge_floor`; a "no need of the wage" quit blocks every
/// employer, not only the one left.
#[test]
fn test_bug6_knowledge_floor_and_secure_quit_block_every_hire() {
    let mut w = world();
    let a = *jobless_housed(&w).first().expect("an adult");
    let role = w.config.trade_role("accountant").expect("the accountant row");
    w.comp_mut::<citysim::Skills>(a).expect("skills").knowledge = 0.0;
    assert!(!citysim::systems::trades::skill_ok(&w, a, role), "knowledge 0.00 keeps no books");
    w.comp_mut::<citysim::Skills>(a).expect("skills").knowledge = 0.5;
    assert!(citysim::systems::trades::skill_ok(&w, a, role));
    assert!(citysim::systems::trades::skill_ok(&w, a, citysim::Role::Farmer), "no floor on an unskilled row");
    let (e1, e2) = {
        let farms = w.buildings_of_kind(citysim::BuildingKind::Farm);
        (farms[0], farms[1])
    };
    life::note_quit(&mut w, a, Some(e1));
    assert!(life::quit_blocks(&w, a, e1) && !life::quit_blocks(&w, a, e2), "a quit blocks its employer");
    life::note_secure_quit(&mut w, a);
    assert!(life::quit_blocks(&w, a, e1) && life::quit_blocks(&w, a, e2), "a secure quit blocks every employer");
}

/// Bug 7: a Club's dancers are tipped (the Club fell through `on_visit`),
/// and the tips go round the tipped staff on duty, not all to the lowest id.
#[test]
fn test_bug7_club_dancers_tipped_in_turn() {
    use citysim::systems::{lod, vice};
    let mut w = world();
    let tipped = |w: &World, s: EntityId| {
        w.comp::<Job>(s).is_some_and(|j| citysim::systems::trades::row_of(w, j.role).is_some_and(|r| r.tipped))
    };
    // A Club with two or more tipped staff sharing a shift.
    let (club, staff, tod) = w
        .buildings_of_kind(citysim::BuildingKind::Club)
        .iter()
        .copied()
        .find_map(|b| {
            let s: Vec<EntityId> = w.staff_of(b).iter().copied().filter(|&s| tipped(&w, s)).collect();
            let start = s.first().and_then(|&x| w.comp::<Job>(x)).and_then(|j| j.shifts.first().map(|x| x.0))?;
            let on: Vec<EntityId> =
                s.into_iter().filter(|&x| w.comp::<Job>(x).is_some_and(|j| j.on_shift(start))).collect();
            (on.len() >= 2).then_some((b, on, start))
        })
        .expect("a Club with two tipped staff on one shift");
    w.tick = 3 * citysim::TICKS_PER_DAY + u64::from(tod);
    for &s in &staff {
        lod::set_lod(&mut w, s, citysim::Lod::Statistical);
    }
    let visitors: Vec<EntityId> = adults(&w).into_iter().filter(|a| !staff.contains(a)).take(12).collect();
    let before: Vec<i64> = staff.iter().map(|&s| w.comp::<citysim::Wallet>(s).expect("w").coins).collect();
    for &v in &visitors {
        set_coins(&mut w, v, 100);
        vice::on_visit(&mut w, v, club, 10);
    }
    let gained =
        staff.iter().zip(&before).filter(|&(&s, &b)| w.comp::<citysim::Wallet>(s).expect("w").coins > b).count();
    assert!(gained >= 2, "the tips went round the dancers on duty ({gained} of {})", staff.len());
}

/// Bug 8: a gang's lead goes to a member of `leader_min_days` before a
/// newer, more loyal one; a night's term (a fare) hands nobody the gang.
#[test]
fn test_bug8_leader_by_tenure_and_no_succession_for_a_night() {
    use citysim::systems::{gang, succession};
    let mut w = world();
    // The seed's gangs start empty and recruit through the first days.
    w.run_ticks(12 * citysim::TICKS_PER_DAY);
    let g = w
        .gangs()
        .into_iter()
        .find(|&g| w.comp::<citysim::Gang>(g).is_some_and(|x| x.leader.is_some() && x.members.len() >= 3))
        .expect("a gang with a leader");
    let members = w.comp::<citysim::Gang>(g).expect("g").members.clone();
    let (vet, new) = (members[0], members[1]);
    w.tick += 40 * citysim::TICKS_PER_DAY;
    let now = w.tick;
    w.comp_mut::<citysim::GangMember>(vet).expect("gm").joined_tick = now - 30 * citysim::TICKS_PER_DAY;
    w.comp_mut::<citysim::GangMember>(new).expect("gm").joined_tick = now - 2 * citysim::TICKS_PER_DAY;
    w.comp_mut::<citysim::Personality>(vet).expect("p").loyalty = 0.1;
    w.comp_mut::<citysim::Personality>(new).expect("p").loyalty = 1.0;
    let scores = gang::leader_scores(&w, g);
    let rank = |a: EntityId| scores.iter().position(|x| x.0 == a).expect("ranked");
    assert!(rank(vet) < rank(new), "a month in outranks two days of loyalty");
    // The leader jailed for a fare's night: no Succession.
    let leader = w.comp::<citysim::Gang>(g).and_then(|x| x.leader).expect("leader");
    w.insert(leader, citysim::Sentence { until_tick: now + 600, crime: citysim::Crime::FareEvasion });
    let before = w.events.iter().filter(|e| e.kind == citysim::EventKind::Succession).count();
    assert!(!succession::decide(&mut w, g, leader));
    let after = w.events.iter().filter(|e| e.kind == citysim::EventKind::Succession).count();
    assert_eq!(before, after, "a night in the cells is no interregnum");
}

fn v1_world(seed: u64) -> World {
    World::new(seed, Config::load().dole_city().v1_profile())
}

/// Bug 9 (shakedowns): a Home whose occupants hold nothing gives nothing,
/// and the empty-handed shakedown makes no Enemy, no WasRobbed memory, no
/// claim and no Shakedown report; the extorter's Friend at home is not
/// squeezed with the rest.
#[test]
fn test_bug9_empty_shakedown_leaves_no_mark_and_spares_a_friend() {
    use citysim::systems::gang;
    let mut w = v1_world(45);
    (w.config.moves.p_min, w.config.moves.p_max) = (1.0, 1.0);
    let g0 = w.gangs()[0];
    let a = w.citizens().into_iter().find(|&id| !w.has::<Job>(id) && w.has::<Brain>(id)).expect("a");
    gang::enlist(&mut w, a, g0);
    let own = w.comp::<citysim::Household>(a).and_then(|h| h.home);
    let home = w
        .buildings_of_kind(citysim::BuildingKind::Home)
        .iter()
        .copied()
        .find(|&h| Some(h) != own && w.comp::<citysim::Building>(h).is_some_and(|b| b.occupants.len() >= 2))
        .expect("an occupied home");
    let occ: Vec<EntityId> = w.comp::<citysim::Building>(home).expect("b").occupants.to_vec();
    for &o in &occ {
        set_coins(&mut w, o, 0);
    }
    assert_eq!(gang::extort(&mut w, a, home), 0);
    for &o in &occ {
        assert!(w.edge(a, o).is_none_or(|e| e.kind != citysim::RelKind::Enemy), "no Enemy for nothing taken");
        let robbed = w.comp::<citysim::Memory>(o).is_some_and(|m| {
            m.entries.iter().any(|e| e.kind == citysim::MemoryKind::WasRobbed && e.subject == Some(a))
        });
        assert!(!robbed, "no WasRobbed for nothing taken");
    }
    assert!(w.comp::<citysim::Building>(home).expect("b").claim.is_none(), "no claim");
    assert!(!w.crime_reports().iter().any(|r| r.suspect == a), "no Shakedown filed");
    // A Friend at home keeps his coins; a stranger pays.
    let (friend, stranger) = (occ[0], occ[1]);
    set_coins(&mut w, friend, 50);
    set_coins(&mut w, stranger, 50);
    {
        let e = w.edge_entry(a, friend);
        e.kind = citysim::RelKind::Friend;
        e.affinity = 0.8;
    }
    assert!(gang::extort(&mut w, a, home) > 0);
    assert_eq!(w.comp::<citysim::Wallet>(friend).expect("w").coins, 50, "the Friend is spared");
    assert!(w.comp::<citysim::Wallet>(stranger).expect("w").coins < 50, "the stranger pays");
}

/// Bug 9 (raids): one raider at the rival's door when the wait is over
/// calls the raid off (1 against 6 after an 8-h march); three go in.
#[test]
fn test_bug9_lone_raider_calls_the_raid_off() {
    use citysim::systems::{gang, raid};
    let mut w = v1_world(49);
    w.config.crime.fight_death_p = 0.0;
    // The shipped floor (the v1 city keeps M11's gang keys, 0).
    w.config.gangs.raid_min_members = Config::load().gangs.raid_min_members;
    assert_eq!(w.config.gangs.raid_min_members, 3);
    let g0 = w.gangs()[0];
    let crew: Vec<EntityId> =
        w.citizens().into_iter().filter(|&id| !w.has::<Job>(id) && w.has::<Brain>(id)).take(3).collect();
    for &m in &crew {
        gang::enlist(&mut w, m, g0);
    }
    let mut full = w.clone();
    let stage = |w: &mut World, id: EntityId, tile: citysim::TilePos| {
        w.abort_plan(id);
        w.leave_building(id);
        let p = w.comp_mut::<citysim::Position>(id).expect("pos");
        p.tile = tile;
        p.building = None;
        w.comp_mut::<Brain>(id).expect("b").current_goal = Some(GoalKind::Raid);
    };
    let door = raid::rival_hideout_tile(&w, crew[0]).expect("a rival door");
    stage(&mut w, crew[0], door);
    let now = w.tick;
    w.comp_mut::<citysim::Gang>(g0).expect("g").raid_at = Some(now);
    assert_eq!(raid::resolve(&mut w, crew[0]), None, "one raider goes home");
    assert!(w.comp::<citysim::Gang>(g0).expect("g").raid_at.is_none(), "the raid is off");
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Raid && e.text.contains("called off")));
    for &m in &crew {
        stage(&mut full, m, door);
    }
    full.comp_mut::<citysim::Gang>(g0).expect("g").raid_at = Some(now);
    assert!(raid::resolve(&mut full, crew[0]).is_some(), "three go in");
}

/// Bug 10 (the edges): a lost scuffle for the cuffs is an arrest, not an
/// assault, in the suspect's memory; a chat with someone now an Enemy does
/// not complete; nobody strips the body of a spouse's parent.
#[test]
fn test_bug10_edges_read_what_just_happened() {
    use citysim::systems::{chrome, law};
    let mut w = world();
    w.config.crime.fight_death_p = 0.0;
    let guard = w.guards()[0];
    let pool: Vec<EntityId> = adults(&w).into_iter().filter(|&x| !law::is_guard(&w, x)).collect();
    // The scuffle: a brave, weak suspect against a strong guard, until he loses one.
    let s = pool[0];
    w.comp_mut::<citysim::Skills>(guard).expect("s").fighting = 1.0;
    w.comp_mut::<citysim::Skills>(s).expect("s").fighting = 0.0;
    let mut lost = false;
    for _ in 0..20 {
        beside(&mut w, guard, s);
        w.comp_mut::<citysim::Personality>(s).expect("p").courage = 0.9;
        law::file_report(&mut w, citysim::Crime::Theft, s, None);
        let n0 = w.comp::<citysim::Memory>(s).map_or(0, |m| m.entries.len());
        law::arrest(&mut w, guard, s);
        let m = w.comp::<citysim::Memory>(s).expect("mem");
        let fresh: Vec<_> = m.entries.iter().skip(n0.min(m.entries.len())).collect();
        if fresh.iter().any(|e| e.kind == citysim::MemoryKind::Won) {
            continue;
        }
        assert!(
            !fresh.iter().any(|e| e.kind == citysim::MemoryKind::Lost && e.subject == Some(guard)),
            "a lost scuffle with the law is no assault"
        );
        lost = fresh.iter().any(|e| e.kind == citysim::MemoryKind::WasArrested && e.subject == Some(guard));
        break;
    }
    assert!(lost, "the suspect lost a scuffle and remembers an arrest");
    // The chat with a new Enemy.
    let (a, b) = (pool[1], pool[2]);
    beside(&mut w, a, b);
    w.edge_entry(a, b).kind = citysim::RelKind::Enemy;
    let now = w.tick;
    let out = citysim::exec::actions::on_complete(&mut w, a, citysim::goap::ActionKind::Chat, Some(b), now, now);
    assert!(matches!(out, citysim::exec::StepResult::Failed(_)), "no chat with an Enemy");
    // The in-law's body.
    let (x, sp) = w
        .citizens()
        .into_iter()
        .find_map(|x| w.spouse_of(x).filter(|_| !law::is_guard(&w, x)).map(|sp| (x, sp)))
        .expect("a married pair");
    let dad = pool.iter().copied().find(|&d| d != x && d != sp && w.edge(x, d).is_none()).expect("a third");
    w.edge_entry(sp, dad).kind = citysim::RelKind::Parent;
    w.kill_by(dad, citysim::DeathCause::Violence, None);
    assert!(w.has::<citysim::Corpse>(dad), "a body");
    assert!(!chrome::may_loot(&w, x, dad), "the spouse's father is kin");
}

/// Bug 10 (the cells): two Enemies jailed together stay Enemies (the
/// drift took -1.00 to -0.10 in 13 h).
#[test]
fn test_bug10_jail_warms_no_enemy() {
    use citysim::systems::law;
    let mut w = world();
    let guard = w.guards()[0];
    let pool: Vec<EntityId> = adults(&w).into_iter().filter(|&x| !law::is_guard(&w, x)).collect();
    let (a, b) = (pool[0], pool[1]);
    for x in [a, b] {
        law::file_report(&mut w, citysim::Crime::Assault, x, None);
        law::jail_suspect(&mut w, guard, x);
        assert!(w.has::<citysim::Sentence>(x));
    }
    {
        let e = w.edge_entry(a, b);
        e.kind = citysim::RelKind::Enemy;
        e.affinity = -1.0;
    }
    w.run_ticks(13 * citysim::TICKS_PER_HOUR);
    let e = w.edge(a, b).expect("edge");
    assert!(e.affinity <= -0.95, "a cell warms no Enemy ({})", e.affinity);
}

/// Bug 11: a corp guard polices its beat (a warrant near its office or a
/// client, not the city's), walks the five clients nearest its office, and
/// a gang member far from the Hideout sleeps elsewhere.
#[test]
fn test_bug11_corp_guard_beat_and_near_hideout() {
    use citysim::systems::{gang, law};
    let mut w = world();
    let pg = w.guards().iter().copied().find(|&g| law::is_private_guard(&w, g)).expect("a private guard");
    let cg = w.guards().iter().copied().find(|&g| law::is_city_guard(&w, g)).expect("a city guard");
    let office = w.comp::<Job>(pg).and_then(|j| j.employer).expect("office");
    let door = w.comp::<citysim::Building>(office).expect("b").door;
    let corp = w.corp_of_building(office).expect("corp");
    let s = adults(&w).into_iter().find(|&x| !law::is_guard(&w, x)).expect("a suspect");
    // Clients: every Home of the city, so more than five, at all distances.
    let homes: Vec<EntityId> = w.buildings_of_kind(citysim::BuildingKind::Home).to_vec();
    let until = w.tick + 100 * citysim::TICKS_PER_DAY;
    w.comp_mut::<citysim::Corp>(corp).expect("corp").contracts = homes.iter().map(|&h| (h, until)).collect();
    let far = homes
        .iter()
        .copied()
        .max_by_key(|&h| w.comp::<citysim::Building>(h).expect("b").door.manhattan(door))
        .expect("a far Home");
    // A warrant far from every client: off the beat; at the office: on it.
    w.comp_mut::<citysim::Corp>(corp).expect("corp").contracts = vec![(office, until)];
    let far_tile = w.comp::<citysim::Building>(far).expect("b").door;
    let now = w.tick;
    w.set_last_seen(s, (far_tile, now));
    assert!(!law::on_beat(&w, pg, s), "a warrant across the city is not the corp's");
    assert!(law::on_beat(&w, cg, s), "the city guard's beat is the city");
    w.set_last_seen(s, (door, now));
    assert!(law::on_beat(&w, pg, s));
    // The route: the five clients nearest the office.
    w.comp_mut::<citysim::Corp>(corp).expect("corp").contracts = homes.iter().map(|&h| (h, until)).collect();
    let route = law::new_patrol_route(&mut w, pg);
    let mut by_dist: Vec<(u32, EntityId)> =
        homes.iter().map(|&h| (w.comp::<citysim::Building>(h).expect("b").door.manhattan(door), h)).collect();
    by_dist.sort();
    let nearest: Vec<EntityId> = by_dist.iter().take(5).map(|&(_, h)| h).collect();
    assert_eq!(route, nearest, "the five clients nearest the office");
    // The Hideout bed: a homeless member beside it holes up; far off, not.
    let g = w.gangs()[0];
    let m = jobless_housed(&w)[0];
    gang::enlist(&mut w, m, g);
    w.comp_mut::<citysim::Household>(m).expect("hh").home = None;
    let hideout = w.hideout_of(g).expect("hideout");
    let hdoor = w.comp::<citysim::Building>(hideout).expect("b").door;
    w.leave_building(m);
    w.comp_mut::<citysim::Position>(m).expect("pos").tile = hdoor;
    assert_eq!(gang::holes_up_at(&w, m), Some(hideout));
    let far_from_hideout = homes
        .iter()
        .map(|&h| w.comp::<citysim::Building>(h).expect("b").door)
        .max_by_key(|t| t.manhattan(hdoor))
        .expect("a far tile");
    assert!(far_from_hideout.manhattan(hdoor) > w.config.gangs.hole_up_max_tiles);
    w.comp_mut::<citysim::Position>(m).expect("pos").tile = far_from_hideout;
    assert_eq!(gang::holes_up_at(&w, m), None, "no bed hours away");
}

/// Bug 12: a worker inside someone else's workplace (a courier's round
/// through a Club) is not at its Workplace (ClerkWork was planned there
/// and failed twice); inside its own, it is.
#[test]
fn test_bug12_another_workplace_is_not_mine() {
    use citysim::goap::world_state::{LocationKey, WorldState};
    let mut w = world();
    let workplace = |w: &World, b: EntityId| {
        w.comp::<citysim::Building>(b).is_some_and(|bd| {
            LocationKey::of_building(bd.kind) == LocationKey::Workplace && bd.kind != citysim::BuildingKind::Home
        })
    };
    let a = day_workers(&w)
        .into_iter()
        .find(|&a| w.comp::<Job>(a).and_then(|j| j.employer).is_some_and(|e| workplace(&w, e)))
        .expect("a worker at a Workplace kind");
    let own = w.comp::<Job>(a).and_then(|j| j.employer).expect("employer");
    let other =
        w.buildings_of_kind(citysim::BuildingKind::Club).iter().copied().find(|&b| b != own).expect("a Club not his");
    w.leave_building(a);
    w.enter_building(a, other);
    assert_ne!(WorldState::observe(&w, a, None).at, LocationKey::Workplace, "a Club on the round is not his post");
    w.leave_building(a);
    w.enter_building(a, own);
    assert_eq!(WorldState::observe(&w, a, None).at, LocationKey::Workplace);
}

/// Bug 13: a Purist member is not sold the chrome that would expel him
/// (a member of a creedless gang in the same state is); a cult preaches
/// under an order other than Expand; a creed gang takes no gray type.
#[test]
fn test_bug13_creeds_bind() {
    use citysim::systems::{assets, creeds, gang, leisure};
    let mut w = world();
    let gangs = w.gangs();
    let (pg, plain) = (gangs[0], gangs[1]);
    w.comp_mut::<citysim::Gang>(pg).expect("g").creed = Some(citysim::word::Creed::Purist);
    w.comp_mut::<citysim::Gang>(plain).expect("g").creed = None;
    let pool = jobless_housed(&w);
    let (a, b) = (pool[0], pool[1]);
    gang::enlist(&mut w, a, pg);
    gang::enlist(&mut w, b, plain);
    for x in [a, b] {
        set_coins(&mut w, x, 5000);
        w.comp_mut::<citysim::Needs>(x).expect("n").wealth = 1.0;
        w.comp_mut::<citysim::Skills>(x).expect("s").hacking = 0.0;
    }
    assert_eq!(creeds::visible_room(&w, a), Some(w.config.creeds.creed_tolerance));
    assert_eq!(creeds::visible_room(&w, b), None);
    let chrome = |w: &World, x: EntityId| {
        assets::shop_choice(w, x, false).is_some_and(|o| o.category == assets::ShopCategory::Chrome)
    };
    assert!(chrome(&w, b), "the control: a creedless member is sold chrome");
    assert!(!chrome(&w, a), "a Purist is not sold the chrome that expels him");
    // The cult's sermon under LieLow (a Purist's waits for Expand).
    let cult = w.config.faith.creeds.iter().position(|_| true).expect("a cult row") as u8;
    w.comp_mut::<citysim::Gang>(plain).expect("g").creed = Some(citysim::word::Creed::Config(cult));
    w.comp_mut::<citysim::Gang>(plain).expect("g").order = citysim::Order::LieLow;
    let share = (w.config.leisure.preach_share * 100.0) as u32;
    let preacher = w
        .citizens()
        .into_iter()
        .find(|&x| {
            x.index % 100 < share && !w.has::<citysim::GangMember>(x) && citysim::systems::demography::is_adult(&w, x)
        })
        .expect("a preacher-to-be");
    gang::enlist(&mut w, preacher, plain);
    assert!(leisure::preacher(&w, preacher), "a cult preaches at LieLow");
    // The gray types are dealt at midnight: none for a creed gang.
    w.jobs2.gray.types.clear();
    w.run_ticks(citysim::TICKS_PER_DAY);
    for g in [pg, plain] {
        if creeds::creed_of(&w, g).is_some() {
            assert!(citysim::systems::gray::gang_type(&w, g).is_none(), "a creed gang is no scav crew");
        }
    }
}

/// Bug 14: the gang's daily stipend is on the books (`Flow::Tribute`), not
/// a bare wallet write the diaries read as a "daily pass".
#[test]
fn test_bug14_stipend_is_a_flow() {
    use citysim::systems::gang;
    let mut w = world();
    let g = w.gangs()[0];
    let pool = jobless_housed(&w);
    for &m in &pool[..3] {
        gang::enlist(&mut w, m, g);
    }
    {
        let x = w.comp_mut::<citysim::Gang>(g).expect("g");
        x.treasury = 1000;
        x.tribute_week = 0;
    }
    let left = citysim::TICKS_PER_DAY - w.tick % citysim::TICKS_PER_DAY;
    w.run_ticks(left + 1);
    let row = w.stats.history.back().expect("a day row").clone();
    let day_flows = row.living.flow_tribute + w.stats.current.living.flow_tribute;
    let stipend = w.config.social.gang_stipend;
    assert!(day_flows >= 3 * stipend, "the stipends ({stipend} x 3) on the books: {day_flows}");
}

/// Bug 15 (the locate): once a Locate is taken, a stranger's sighting no
/// longer pays (the crowd filled the cap and the taker who staked out three
/// days got 0 c); the taker's (`on_contact`, the ledger's report) still do.
#[test]
fn test_bug15_taken_locate_pays_the_taker_only() {
    use citysim::contract::{ContractKind, ContractStatus, Origin, Posting, Target};
    use citysim::systems::contracts;
    let mut w = world();
    w.config.bounty.min_gap_hours = 0;
    let corp = w.corps()[0];
    w.comp_mut::<citysim::Corp>(corp).expect("corp").treasury = 100_000;
    let pool: Vec<EntityId> = adults(&w).into_iter().filter(|&x| !citysim::systems::law::is_guard(&w, x)).collect();
    let (t, taker, crowd) = (pool[0], pool[1], pool[2]);
    let p = Posting {
        buyer: Some(corp),
        agent: None,
        kind: ContractKind::Locate,
        target: Target::Agent(t),
        broker: None,
        deadline_days: 7,
        origin: Origin::God,
        price: None,
    };
    let id = contracts::post(&mut w, p).expect("posted");
    let tile = w.comp::<citysim::Position>(t).expect("pos").tile;
    let paid = |w: &World| w.contracts[&id].paid_sightings;
    assert!(contracts::sees(&w, crowd, id), "the crowd sees the open record");
    contracts::on_sighting(&mut w, crowd, t, tile);
    assert_eq!(paid(&w), 1, "the control: an open Locate pays the crowd");
    contracts::god_take(&mut w, id, taker).expect("taken");
    assert_eq!(w.contracts[&id].status, ContractStatus::Taken);
    w.tick += 1;
    contracts::on_sighting(&mut w, crowd, t, tile);
    assert_eq!(paid(&w), 1, "a taken Locate pays no stranger");
}

/// Bug 15 (the marked man): word of a Hit on oneself, however faint, is a
/// grudge on the buyer and a fright (it weighed 0.25 under `grudge_min`).
#[test]
fn test_bug15_marked_man_reacts() {
    let mut w = world();
    let ad = adults(&w);
    let (holder, buyer) = (ad[0], ad[1]);
    w.comp_mut::<citysim::Needs>(holder).expect("n").safety = 1.0;
    let r = citysim::word::DeedRef { deed: citysim::word::Deed::Hired, actor: Some(buyer), object: Some(holder) };
    citysim::systems::grudges::on_learn(&mut w, holder, &r, 0.3, 2);
    let held = w.comp::<citysim::word::Grudges>(holder).is_some_and(|g| g.list.iter().any(|x| x.target == buyer));
    assert!(held, "a grudge on the man who paid for the hit");
    let safety = w.comp::<citysim::Needs>(holder).expect("n").safety;
    assert!(safety <= 1.0 - w.config.grudges.marked_safety_drop + 1e-6, "afraid ({safety})");
}

/// Found under batch 3 (a save that diverged): a spouse turned on stays a
/// spouse in the edge as in `World::spouses`, so a loaded world rebuilds
/// the same marriage the running one holds.
#[test]
fn test_wronged_spouse_index_survives_a_load() {
    let mut w = world();
    let (a, s) = w.citizens().into_iter().find_map(|x| w.spouse_of(x).map(|s| (x, s))).expect("a married pair");
    citysim::systems::social::robbed_by(&mut w, a, s);
    assert_eq!(w.spouse_of(a), Some(s), "still married in the running world");
    let back = citysim::save::from_ron(&citysim::save::to_ron(&w)).expect("loads");
    assert_eq!(back.spouse_of(a), Some(s), "and in the loaded one");
    assert!(w.edge(a, s).is_some_and(|e| e.affinity < 0.0), "the wrong is felt");
}
