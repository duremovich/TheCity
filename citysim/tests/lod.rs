//! M7: level of detail, the Statistical tick and replay (the ticks/s floor is
//! `core_sanity`'s, tests/core.rs).

use citysim::systems::lod;
use citysim::{
    Brain, Config, ExecState, Household, Lod, Needs, PlayerCommand, Position, Sentence, World, TICKS_PER_DAY,
    TICKS_PER_HOUR,
};

fn world(seed: u64) -> World {
    World::new(seed, Config::load().v1_profile())
}

fn counts(w: &World) -> (usize, usize, usize) {
    let mut n = (0, 0, 0);
    for id in w.citizens() {
        match w.comp::<Brain>(id).map(|b| b.lod) {
            Some(Lod::Full) => n.0 += 1,
            Some(Lod::Coarse) => n.1 += 1,
            Some(Lod::Statistical) => n.2 += 1,
            None => {}
        }
    }
    n
}

#[test]
fn test_lod_assignment_counts() {
    let mut w = world(61);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let (full, coarse, stat) = counts(&w);
    let jailed = w.citizens().into_iter().filter(|&id| w.has::<Sentence>(id)).count();
    assert!(full <= w.config.lod.max_full, "full {full}");
    assert!(coarse <= w.config.lod.max_coarse + jailed, "coarse {coarse}");
    assert!(stat > 0, "nobody statistical");
    assert_eq!(full + coarse + stat, w.citizens().into_iter().filter(|&id| w.has::<Brain>(id)).count());
}

#[test]
fn test_pinned_agent_is_full() {
    let mut w = world(62);
    w.run_ticks(TICKS_PER_HOUR + 1);
    let id = w
        .citizens()
        .into_iter()
        .find(|&id| w.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Statistical))
        .expect("a statistical agent");
    w.push_command(PlayerCommand::Pin(id, true));
    w.run_ticks(TICKS_PER_HOUR);
    assert_eq!(w.comp::<Brain>(id).map(|b| b.lod), Some(Lod::Full));
    assert!(w.comp::<Brain>(id).is_some_and(|b| b.pinned));
}

#[test]
fn test_demote_full_to_coarse_keeps_plan() {
    let mut w = world(63);
    // Find a Full agent mid-walk.
    let mut found = None;
    for _ in 0..(4 * TICKS_PER_HOUR) {
        w.tick();
        found = w.citizens().into_iter().find(|&id| {
            w.comp::<Brain>(id)
                .is_some_and(|b| b.lod == Lod::Full && b.plan.is_some() && matches!(b.exec, ExecState::Goto { .. }))
        });
        if found.is_some() {
            break;
        }
    }
    let id = found.expect("a walking Full agent");
    let (plan, step) = {
        let b = w.comp::<Brain>(id).expect("brain");
        (b.plan.clone(), b.plan_step)
    };
    lod::set_lod(&mut w, id, Lod::Coarse);
    let b = w.comp::<Brain>(id).expect("brain");
    assert_eq!(b.lod, Lod::Coarse);
    assert_eq!(b.plan, plan, "plan kept");
    assert_eq!(b.plan_step, step, "step kept");
    assert!(matches!(b.exec, ExecState::GotoTimed { .. }), "walk became a timed arrival: {:?}", b.exec);
}

#[test]
fn test_demote_to_statistical_snaps_home_at_night() {
    let mut w = world(64);
    w.tick = 100; // 01:40, Night
    let id = w.citizens().into_iter().find(|&id| w.comp::<Brain>(id).is_some_and(|b| b.lod == Lod::Coarse)).expect("c");
    let home = w.comp::<Household>(id).and_then(|h| h.home).expect("home");
    let door = w.comp::<citysim::Building>(home).expect("b").door;
    // Put them somewhere else first.
    w.leave_building(id);
    w.comp_mut::<Position>(id).expect("pos").tile = citysim::TilePos { x: 3, y: 3 };
    lod::set_lod(&mut w, id, Lod::Statistical);
    let pos = w.comp::<Position>(id).expect("pos");
    assert_eq!(pos.tile, door);
    assert_eq!(pos.building, None);
    assert_eq!(w.comp::<Brain>(id).map(|b| b.lod), Some(Lod::Statistical));
}

#[test]
fn test_statistical_hourly_decay_equals_60_ticks() {
    let cfg = Config::load().needs;
    let ctx =
        citysim::needs::DecayCtx { sociability: 0.5, season_energy_mult: 1.0, ..citysim::needs::DecayCtx::plain() };
    let start = Needs {
        hunger: 0.8,
        energy: 0.7,
        safety: 0.6,
        wealth: 0.3,
        belonging: 0.5,
        intimacy: 0.4,
        starving_since: None,
        fun: 1.0,
        status: 1.0,
    };
    let mut hourly = start.clone();
    citysim::needs::decay(&mut hourly, &cfg, &ctx, TICKS_PER_HOUR as u32);
    let mut stepped = start;
    for _ in 0..TICKS_PER_HOUR {
        citysim::needs::decay(&mut stepped, &cfg, &ctx, 1);
    }
    for (a, b) in [
        (hourly.hunger, stepped.hunger),
        (hourly.energy, stepped.energy),
        (hourly.safety, stepped.safety),
        (hourly.belonging, stepped.belonging),
        (hourly.intimacy, stepped.intimacy),
    ] {
        assert!((a - b).abs() < 1e-5, "{a} vs {b}");
    }
}

/// Parity v2 (M10 § 7, D37): Full vs Statistical in `calibrate`'s city
/// (`Config::calibration_city` at `[world] population`: the shipped city,
/// gangless, riot- and asset-free; 500 residents until the 2026-10-09
/// recalibration), 30 days, seeds 2000-2005 pooled. Thefts, hunger-days, arrests,
/// assaults, violent deaths and marriages per 100 agent-days within 15%
/// (floor 0.5); then, with every hole bound, the share of attributed crimes
/// per actor lawfulness bucket within `max(0.15 x share, 0.05)`. Jobs &
/// lives phase 7b: all paid venue visits and coins per 100 adult-days
/// within 15 % too; the synth share of meals and the per-kind venue rows
/// printed as FINDINGs. Run with `--ignored` (nightly, docs/TESTING.md:
/// the one parity test kept).
#[test]
#[ignore = "tier: milestone"]
fn test_full_vs_statistical_within_15pct() {
    use citysim::systems::bind;
    use citysim::{EventKind, Personality};
    let agents: u32 = Config::load().world.population;
    const DAYS: u64 = 30;
    const SEEDS: [u64; 6] = [2000, 2001, 2002, 2003, 2004, 2005];
    struct Run {
        rates: [f64; 11],
        actors: [f64; 3],
        // Jobs & lives phase 7b: paid visits and their coins per venue kind
        // (`CoinProbe::venue_paid`), per 100 agent-days.
        venues: [[f64; 2]; VENUE_KINDS.len()],
        // All paid venue visits and their coins per 100 adult-days.
        paid: [f64; 2],
        // Lives at work phase 1 (§ 6): service encounters and the staffed
        // shifts of serving rows (world-side by construction; the row is
        // encounters per staffed shift, pooled over the seeds).
        encounters: [f64; 2],
        // Lives at work phase 3 (§ 6): the super's door rent and all rent
        // paid (the door rent share by tier, printed).
        door: [f64; 2],
        // Lives at work phase 4a (§ 6): cab and rickshaw rides (paid or
        // dodged) and the commutes (the cab rides per commuter-day by tier).
        cab: [f64; 2],
    }
    // The paid-leisure mirror (Jobs & lives phase 7b): the venue kinds of
    // the calibration city's ladder.
    const VENUE_KINDS: [&str; 9] =
        ["Arcade", "Bar", "Brothel", "Club", "Den", "FightPit", "Lounge", "NoodleBar", "StripClub"];
    fn bucket(w: &World, id: citysim::EntityId) -> Option<usize> {
        let l = w.comp::<Personality>(id)?.lawfulness;
        Some(if l < 0.3 {
            0
        } else if l < 0.7 {
            1
        } else {
            2
        })
    }
    fn run(force: Lod, seed: u64, agents: u32) -> Run {
        // The city `calibrate` measured: gangless, full Warehouse.
        let mut cfg = Config::load().calibration_city(agents);
        cfg.lod.force = Some(force);
        cfg.lod.stat_violence_mult = 1.0;
        // The learned-policy experiment: `CITYSIM_STAT_POLICY=mlp`.
        if let Ok(p) = std::env::var("CITYSIM_STAT_POLICY") {
            cfg.lod.policy = p;
        }
        let mut w = World::new(seed, cfg);
        let mut hunger_days = 0u64;
        let (mut assaults, mut marriages) = (0u64, 0u64);
        let mut actors = [0u64; 3];
        let mut cursor = 0u64;
        // Actor buckets are read when the event is seen (end of its day).
        let mut split = [[0u64; 3]; 2];
        let mut scan = |w: &World, cursor: &mut u64, assaults: &mut u64, marriages: &mut u64, actors: &mut [u64; 3]| {
            for e in w.events.iter().filter(|e| e.id >= *cursor) {
                match e.kind {
                    EventKind::Assault | EventKind::Assaulted => *assaults += 1,
                    EventKind::Marriage => *marriages += 1,
                    _ => {}
                }
                let actor_side = match force {
                    Lod::Full => matches!(e.kind, EventKind::Theft | EventKind::Assault | EventKind::Murder),
                    // A Full robbery is one Theft by its thief. Off screen
                    // the thief's own `p_steal` roll is that Theft, and the
                    // victim's `p_robbed` hole is the same robbery seen from
                    // the other side: counting its bound actor too counted
                    // every robbery twice, at the binder's (1 - l)^2 weights
                    // (M10 phase 5b, once the fed calibration city's thefts
                    // became mostly robberies).
                    //
                    // Assaults and murders count on both sides: a forced
                    // Statistical city keeps guards, gravediggers and the
                    // wanted as bodies, and their fights are Full events with
                    // a named actor (an off-screen victim's Murder names
                    // nobody, so `bucket` skips it). Counting only thefts
                    // here dropped the bodies' violence from this side alone
                    // (M10 phase 5c).
                    _ => {
                        matches!(e.kind, EventKind::Theft | EventKind::Assault | EventKind::Murder)
                            || (e.kind == EventKind::Attributed
                                && e.actors.len() == 2
                                && !e.text.contains(citysim::HoleKind::Robbed.noun()))
                    }
                };
                if actor_side {
                    if let Some(b) = e.actors.first().and_then(|&a| bucket(w, a)) {
                        actors[b] += 1;
                        // Thefts apart from assaults, murders and bound holes, for the diagnosis.
                        let kind = usize::from(e.kind != EventKind::Theft);
                        split[kind][b] += 1;
                    }
                }
            }
            *cursor = w.next_event_id;
        };
        let mut adult_days = 0u64;
        for _ in 0..DAYS {
            w.run_ticks(TICKS_PER_DAY);
            adult_days += w
                .citizens()
                .into_iter()
                .filter(|&id| w.has::<Brain>(id) && citysim::systems::demography::is_adult(&w, id))
                .count() as u64;
            hunger_days +=
                w.citizens().into_iter().filter(|&id| w.comp::<Needs>(id).is_some_and(|n| n.hunger < 0.2)).count()
                    as u64;
            scan(&w, &mut cursor, &mut assaults, &mut marriages, &mut actors);
        }
        bind::bind_all(&mut w);
        scan(&w, &mut cursor, &mut assaults, &mut marriages, &mut actors);
        let pop: [usize; 3] =
            std::array::from_fn(|b| w.citizens().into_iter().filter(|&id| bucket(&w, id) == Some(b)).count());
        eprintln!(
            "{force:?} seed {seed}: thefts by bucket {:?}, violence/attributed by bucket {:?}, population {pop:?}",
            split[0], split[1]
        );
        let per = f64::from(agents) * DAYS as f64 / 100.0;
        let sum = |f: fn(&citysim::DayRow) -> u32| w.stats.history.iter().map(f).sum::<u32>() as f64 / per;
        Run {
            rates: [
                sum(|r| r.thefts),
                hunger_days as f64 / per,
                sum(|r| r.arrests),
                assaults as f64 / per,
                sum(|r| r.deaths_violence),
                marriages as f64 / per,
                // Hunger as sickness § 5 (zeros with `[sickness]` off): the
                // synth share of meals (percent), bin meals per 100
                // agent-days, mean malnutrition on the last day (x 100).
                {
                    let meals = |f: fn(&citysim::DayRow) -> u32| w.stats.history.iter().map(f).sum::<u32>() as f64;
                    let all = meals(|r| {
                        r.health.meals_real + r.health.meals_synth + r.health.meals_dive + r.health.meals_alms
                    });
                    100.0 * meals(|r| r.health.meals_synth + r.health.meals_alms) / all.max(1.0)
                },
                sum(|r| r.health.meals_dive),
                100.0 * f64::from(w.stats.history.back().map_or(0.0, |r| r.health.mean_malnutrition)),
                // Transit phase 2 (zeros with the El off): commute rides per
                // 100 agent-days (a body's ride to work counted twice for the
                // way back, as a Statistical riding workday pays two; errand
                // rides are not counted on either side), and the ride share
                // of commutes (x 100).
                {
                    let t = |f: fn(&citysim::stats::TransitCols) -> u32| {
                        w.stats.history.iter().map(|r| f64::from(f(&r.transit))).sum::<f64>()
                    };
                    (2.0 * t(|c| c.commute_rides) + t(|c| c.rides_stat)) / per
                },
                {
                    let t = |f: fn(&citysim::stats::TransitCols) -> u32| {
                        w.stats.history.iter().map(|r| f64::from(f(&r.transit))).sum::<f64>()
                    };
                    let all = t(|c| c.commutes) + t(|c| c.stat_commutes);
                    100.0 * (t(|c| c.commute_rides) + t(|c| c.stat_commute_rides)) / all.max(1.0)
                },
            ],
            actors: actors.map(|n| n as f64),
            venues: VENUE_KINDS.map(|k| {
                let (n, c) = w.probe.venue_paid.get(k).copied().unwrap_or((0, 0));
                [n as f64 / per, c as f64 / per]
            }),
            paid: {
                let (n, c) = w.probe.venue_paid.values().fold((0u64, 0i64), |a, v| (a.0 + v.0, a.1 + v.1));
                let ad = adult_days.max(1) as f64 / 100.0;
                [n as f64 / ad, c as f64 / ad]
            },
            encounters: {
                let j = |f: fn(&citysim::stats::Jobs2Cols) -> u32| {
                    w.stats.history.iter().map(|r| r.jobs2.as_ref().map_or(0.0, |j| f64::from(f(j)))).sum::<f64>()
                };
                [j(|c| c.served_total()), j(|c| c.serve_shifts)]
            },
            door: [
                w.stats.history.iter().map(|r| r.jobs2.as_ref().map_or(0.0, |j| j.door_rent as f64)).sum::<f64>(),
                w.stats.history.iter().map(|r| r.rent_paid as f64).sum::<f64>(),
            ],
            cab: {
                let j = |f: fn(&citysim::stats::Jobs2Cols) -> u32| {
                    w.stats.history.iter().map(|r| r.jobs2.as_ref().map_or(0.0, |j| f64::from(f(j)))).sum::<f64>()
                };
                let t = |f: fn(&citysim::stats::TransitCols) -> u32| {
                    w.stats.history.iter().map(|r| f64::from(f(&r.transit))).sum::<f64>()
                };
                [j(|c| c.cab_trips + c.rickshaw_trips + c.fares_short), t(|c| c.commutes) + t(|c| c.stat_commutes)]
            },
        }
    }
    // Six cities per tier, pooled: one 30-day city's theft rate varies by a
    // quarter from seed to seed (seed 2000 alone runs 4.1 against 2.9-3.6),
    // and the actor shares, which a few repeat thieves dominate, moved 0.04
    // on the Full side alone between seeds 2000-2002 and 2003-2005 (law0
    // 0.204 vs 0.242) against a 0.05 tolerance (M10 review fixes).
    let pooled = |force: Lod| {
        let runs: Vec<Run> = SEEDS.iter().map(|&s| run(force, s, agents)).collect();
        let n = runs.len() as f64;
        let rates = std::array::from_fn(|i| runs.iter().map(|r| r.rates[i]).sum::<f64>() / n);
        let counts: [f64; 3] = std::array::from_fn(|b| runs.iter().map(|r| r.actors[b]).sum::<f64>());
        let total = counts.iter().sum::<f64>().max(1.0);
        let venues =
            std::array::from_fn(|k| std::array::from_fn(|j| runs.iter().map(|r| r.venues[k][j]).sum::<f64>() / n));
        let paid = std::array::from_fn(|j| runs.iter().map(|r| r.paid[j]).sum::<f64>() / n);
        let encounters = std::array::from_fn(|j| runs.iter().map(|r| r.encounters[j]).sum::<f64>());
        let door = std::array::from_fn(|j| runs.iter().map(|r| r.door[j]).sum::<f64>());
        let cab = std::array::from_fn(|j| runs.iter().map(|r| r.cab[j]).sum::<f64>());
        Run { rates, actors: counts.map(|c| c / total), venues, paid, encounters, door, cab }
    };
    let full = pooled(Lod::Full);
    let stat = pooled(Lod::Statistical);
    let names = [
        "thefts",
        "hunger-days",
        "arrests",
        "assaults",
        "violent deaths",
        "marriages",
        "synth % meals",
        "bin meals",
        "malnutrition x100",
        "commute rides",
        "ride share x100",
    ];
    // Jobs & lives phase 7b: the synth share of meals is a printed FINDING,
    // not an assert. The off-screen share runs ~5-6 points over the Full
    // tier's by an intentional mirror deviation: M10's `stat_store_food`
    // puts a Statistical buyer's leftovers in the shared Home pantry
    // (where a broke housemate eats them), where a Full buyer carries its
    // own (`EatFromInventory`, 70 % of Full meals). A poor buyer's synth
    // then feeds the working housemates: on the calibration city (seeds
    // 2000-2002) the employed with 10+ coins ate 14 % synth off screen
    // against 7.8 % on screen, the poor employed 47 % against 32 %, the poor
    // jobless 84 % against 83 %. Keeping the leftovers carried closed it
    // (43.4 against 38.2) and opened the bin meals row (1.94 against
    // 1.20). The gap was 5.28 at 05b7c15 (43.60 against 48.88). The health
    // outcomes it could hide (hunger-days, malnutrition, bin meals) stay
    // asserted.
    const FINDING_ROWS: [&str; 1] = ["synth % meals"];
    let mut failures = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let (f, s) = (full.rates[i], stat.rates[i]);
        let tolerance = (0.15 * f).max(0.5);
        if FINDING_ROWS.contains(name) {
            eprintln!(
                "FINDING {name:15} Full {f:7.3}  Statistical {s:7.3}  per 100 agent-days (printed, M10 pantry rule)"
            );
            continue;
        }
        eprintln!("{name:15} Full {f:7.3}  Statistical {s:7.3}  per 100 agent-days (tolerance {tolerance:.2})");
        if (f - s).abs() > tolerance {
            failures.push(format!("{name}: Full {f:.3} vs Statistical {s:.3}"));
        }
    }
    // The paid-leisure mirror (the Statistical evening's learned rates,
    // `StatTable::p_venue`): all paid venue visits and their coins per 100
    // adult-days asserted; by venue kind printed as FINDINGs (a kind of ~100
    // visits a seed, the Lounge, varies past 15 % between seeds alone).
    for (j, what) in ["paid venue visits", "paid venue coins"].iter().enumerate() {
        let (f, s) = (full.paid[j], stat.paid[j]);
        let tolerance = (0.15 * f).max(0.5);
        eprintln!("{what:17} Full {f:7.3}  Statistical {s:7.3}  per 100 adult-days (tolerance {tolerance:.2})");
        if (f - s).abs() > tolerance {
            failures.push(format!("{what}: Full {f:.3} vs Statistical {s:.3}"));
        }
    }
    for (k, kind) in VENUE_KINDS.iter().enumerate() {
        for (j, what) in ["visits", "coins"].iter().enumerate() {
            let (f, s) = (full.venues[k][j], stat.venues[k][j]);
            let name = format!("{kind} {what}");
            eprintln!("FINDING {name:17} Full {f:7.3}  Statistical {s:7.3}  per 100 agent-days");
        }
    }
    for b in 0..3 {
        let (f, s) = (full.actors[b], stat.actors[b]);
        let tolerance = (0.15 * f).max(0.05);
        eprintln!("actor share law{b}  Full {f:.3}  Statistical {s:.3}  (tolerance {tolerance:.3})");
        if (f - s).abs() > tolerance {
            failures.push(format!("actor share law{b}: Full {f:.3} vs Statistical {s:.3}"));
        }
    }
    // Lives at work phase 1 (§ 6): encounters per staffed shift of a
    // serving row, pooled over the seeds. The pass is world-side, but the
    // presence rule is not symmetric: a Full server must be inside the
    // building (a meal, a walk or a late commute loses the hour's
    // arrivals) where a Statistical one is credited by the clock
    // (`lod::shift_attended`). Read 0.707 Full against 0.877 Statistical on
    // the phase 1 tree (24 % apart): a printed FINDING, not an assert, with
    // the reason (CLAUDE.md rule 2; the gate decides whether a Full server
    // on shift is credited while momentarily out). Zeros with the encounter off.
    {
        let per_shift = |r: &Run| if r.encounters[1] > 0.0 { r.encounters[0] / r.encounters[1] } else { 0.0 };
        let (f, s) = (per_shift(&full), per_shift(&stat));
        eprintln!(
            "FINDING encounters/shift  Full {f:7.3}  Statistical {s:7.3}  ({} / {} shifts; the Full server must be \
             inside, the Statistical one is credited by the clock)",
            full.encounters[1], stat.encounters[1]
        );
        // Lives at work phase 3 (§ 6): the super's door rent as a share of
        // all rent paid, by tier (the same settle moved earlier; printed).
        let share = |r: &Run| if r.door[1] > 0.0 { r.door[0] / r.door[1] } else { 0.0 };
        eprintln!(
            "FINDING door rent share  Full {:7.3}  Statistical {:7.3}  ({:.0} / {:.0} of rent paid)",
            share(&full),
            share(&stat),
            full.door[0],
            stat.door[0]
        );
        // Lives at work phase 4a (§ 6): cab rides per commuter-day by tier
        // (one roll, `cab_roll`, for both; a Full lawless rider may dodge,
        // counted, where `stat_cab` always pays; printed).
        let per = |r: &Run| if r.cab[1] > 0.0 { 100.0 * r.cab[0] / r.cab[1] } else { 0.0 };
        eprintln!(
            "FINDING cab rides/100 commutes  Full {:7.3}  Statistical {:7.3}  ({:.0} / {:.0} rides)",
            per(&full),
            per(&stat),
            full.cab[0],
            stat.cab[0]
        );
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn test_command_log_replay_matches() {
    let mut a = world(65);
    let who = a.citizens().into_iter().find(|&id| a.has::<Brain>(id)).expect("agent");
    let cmds = [
        (100, PlayerCommand::SetTaxRate(0.1)),
        (500, PlayerCommand::GrantCoins { agent: who, amount: 20 }),
        (900, PlayerCommand::ReleaseReserve { amount: 100 }),
        (1300, PlayerCommand::Pin(who, true)),
        (1700, PlayerCommand::SetDolePerDay(4)),
    ];
    let end = 2 * TICKS_PER_DAY;
    for (t, cmd) in &cmds {
        while a.tick < *t {
            a.tick();
        }
        a.push_command(cmd.clone());
    }
    while a.tick < end {
        a.tick();
    }
    assert_eq!(a.command_log.len(), 5);
    let saved = citysim::save::to_ron(&a);

    let mut b = World::replay(65, Config::load().v1_profile(), &a.command_log, end);
    assert_eq!(b.tick, end);
    assert_eq!(citysim::save::to_ron(&b), saved, "replay diverged");
    b.tick();
}

/// Jobs and room P2 fix (seed 44's wages-city wave: 345 Statistical agents
/// starved beside an empty Market while the other two held 1,200 each): a
/// hungry Statistical agent whose Market is empty buys, or steals when
/// starving and broke, at a stocked one, and the price pick passes the empty
/// shelf over. Only the Statistical pass runs, for one hour (one turn each).
#[test]
fn test_statistical_agent_eats_at_a_stocked_market_when_its_own_is_empty() {
    use citysim::{Building, BuildingKind, EventKind, Inventory, Job, Wallet};
    for broke in [false, true] {
        let mut w = World::new(42, Config::load().dole_city());
        w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
        let id = w
            .tier(Lod::Statistical)
            .iter()
            .copied()
            .find(|&a| {
                citysim::systems::demography::is_adult(&w, a)
                    && !w.has::<Sentence>(a)
                    && w.comp::<Job>(a).and_then(|j| j.employer).is_none()
            })
            .expect("a jobless Statistical adult");
        let empty = w.local(id, BuildingKind::Market).expect("a Market");
        w.comp_mut::<Building>(empty).expect("market").stock_food = 0;
        let other = w.local(id, BuildingKind::Market).expect("a Market");
        assert_ne!(other, empty, "the price pick passes the empty shelf over");
        let stock = w.comp::<Building>(other).map_or(0, |b| b.stock_food);
        assert!(stock > 0);
        assert_eq!(w.food_market(id), Some(other));
        // The meal's mechanics, not its delay: the M10 rule (every hungry
        // hour eats), not the table's learned `p_meal`.
        if let Some(t) = w.stat_table.as_mut() {
            t.p_meal = None;
        }
        // Homeless: no pantry a housemate could refill within the hour.
        w.set_home(id, None);
        w.comp_mut::<Inventory>(id).expect("inv").food = 0;
        w.comp_mut::<Wallet>(id).expect("wallet").coins = if broke { 0 } else { 100 };
        if broke {
            // No dole to buy with: the starving agent's only meal is a theft.
            w.levers.dole_per_day = 0;
            // The M10 rule (a table without `p_desperate`: a starving adult
            // steals whatever its lawfulness); the table's `p_desperate`
            // roll is the no-net tests' below.
            if let Some(t) = w.stat_table.as_mut() {
                t.p_desperate = None;
                // Nor a scavenged or begged coin to buy with (the earn path; on
                // the transit phase 3 table this agent scavenged its meal).
                t.p_scavenge = None;
                t.p_beg = None;
            }
        }
        {
            let n = w.comp_mut::<Needs>(id).expect("needs");
            n.hunger = if broke { 0.0 } else { 0.2 };
            n.starving_since = None;
        }
        let start = w.events.back().map_or(0, |e| e.id + 1);
        for _ in 0..TICKS_PER_HOUR {
            lod::run_statistical(&mut w);
            w.tick += 1;
        }
        assert!(w.comp::<Needs>(id).is_some_and(|n| n.hunger > 0.2), "broke {broke}: the agent ate");
        assert_eq!(w.comp::<Building>(empty).map(|b| b.stock_food), Some(0));
        let left = w.comp::<Building>(other).map_or(0, |b| b.stock_food);
        assert!(left < stock, "broke {broke}: the food came from the stocked Market");
        if broke {
            let theft = w
                .events
                .iter()
                .any(|e| e.id >= start && e.kind == EventKind::Theft && e.actors.as_slice() == [id, other]);
            assert!(theft, "the starving thief stole at the stocked Market");
        }
    }
}

/// The flip-readiness round's no-net city at day 1, 10:00, and a jobless
/// homeless Statistical adult in it with nothing to eat and no coins (no
/// pantry a housemate could refill within the hour). The table's no-net
/// rates are set by each test.
fn no_net_stat_agent() -> (World, citysim::EntityId) {
    use citysim::{Inventory, Job, Wallet};
    let cfg = Config::load();
    let mut w = World::new(42, cfg);
    assert!(citysim::systems::econ::wages_on(&w) && !citysim::systems::policy::net_enacted(&w));
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    let id = w
        .tier(Lod::Statistical)
        .iter()
        .copied()
        .find(|&a| {
            citysim::systems::demography::is_adult(&w, a)
                && !w.has::<Sentence>(a)
                && w.comp::<Job>(a).and_then(|j| j.employer).is_none()
        })
        .expect("a jobless Statistical adult");
    w.set_home(id, None);
    w.comp_mut::<Inventory>(id).expect("inv").food = 0;
    w.comp_mut::<Inventory>(id).expect("inv").stolen_food = 0;
    w.comp_mut::<Wallet>(id).expect("wallet").coins = 0;
    if let Some(t) = w.stat_table.as_mut() {
        t.p_scavenge = Some([0.0; citysim::STAT_ROWS]);
        t.p_beg = Some([0.0; citysim::STAT_ROWS]);
        t.p_desperate = Some([0.0; 3]);
    }
    (w, id)
}

/// One hour of the Statistical pass alone (each agent's one turn).
fn stat_hour(w: &mut World) {
    for _ in 0..TICKS_PER_HOUR {
        lod::run_statistical(w);
        w.tick += 1;
    }
}

/// Flip readiness, the no-net income path: a broke jobless Statistical
/// adult scavenges on its row's `p_scavenge` and the find is paid from the
/// Recycler's till (`jobs::scavenge_find`, Full's Scavenge); a table without
/// the rate (the dole city's) never scavenges off screen.
#[test]
fn test_no_net_statistical_scavenge_is_paid_from_the_till() {
    use citysim::Wallet;
    for rated in [true, false] {
        let (mut w, id) = no_net_stat_agent();
        w.config.life.scavenge_p = 1.0;
        // Every broke jobless Statistical adult scavenges this hour: a till
        // deep enough for all of them.
        w.econ.recycler_till = 1_000_000;
        w.comp_mut::<Needs>(id).expect("needs").hunger = 1.0;
        if let Some(t) = w.stat_table.as_mut() {
            t.p_scavenge = rated.then_some([1.0; citysim::STAT_ROWS]);
        }
        let till = w.econ.recycler_till;
        stat_hour(&mut w);
        let coins = w.comp::<Wallet>(id).map_or(0, |x| x.coins);
        let paid = w.config.treasury.scrap_coins;
        if rated {
            assert_eq!(coins, paid, "the hour's find paid {paid}");
            assert!(w.econ.recycler_till <= till - paid, "from the till");
        } else {
            assert_eq!(coins, 0, "no rate, no off-screen scavenging");
        }
    }
}

/// Parity (the bin meals red): the off-screen income path has the Full
/// Scavenge's and Beg's gate (the planner's `ctx.poor`), not the Earn
/// goal's savings line: a Full jobless adult with a meal's coins (and,
/// homeless, a Hotel night's) neither scavenged nor begged (seed 2000,
/// all-Full: 0.196 scavenges per hour at 0 coins, 0.012 at 1, under 0.01
/// above), so off screen it does not either; a coin short of the line, it
/// does. Shadow V4 bug 1: the gate is `life::earn_short` (with no dole, under
/// `[life] earn_meals_no_net` meals' coins too), the line its edge.
#[test]
fn test_statistical_scavenge_only_when_broke() {
    use citysim::Wallet;
    for short in [false, true] {
        let (mut w, id) = no_net_stat_agent();
        w.config.life.scavenge_p = 1.0;
        w.econ.recycler_till = 1_000_000;
        w.comp_mut::<Needs>(id).expect("needs").hunger = 1.0;
        let price = w.local(id, citysim::BuildingKind::Market).map_or(1, |m| w.price_for(m, id));
        let line = (citysim::systems::life::meal_price(&w, id) + w.config.street.night_price)
            .max(w.config.life.earn_meals_no_net * price);
        assert!(line < citysim::goap::world_state::SAVINGS_DAYS * price, "below the savings line either way");
        w.comp_mut::<Wallet>(id).expect("wallet").coins = if short { line - 1 } else { line };
        assert_eq!(citysim::systems::life::earn_short(&w, id), short);
        if let Some(t) = w.stat_table.as_mut() {
            t.p_scavenge = Some([1.0; citysim::STAT_ROWS]);
            // The near-broke's own rate (Shadow V4 bug 1's table field).
            t.p_scavenge_short = Some([1.0; citysim::STAT_ROWS]);
        }
        let till = w.econ.recycler_till;
        stat_hour(&mut w);
        let after = w.comp::<Wallet>(id).map_or(0, |x| x.coins);
        if short {
            assert!(after >= line - 1 + w.config.treasury.scrap_coins, "broke: the hour's find ({after})");
            assert!(w.econ.recycler_till < till, "paid from the till");
        } else {
            assert!(after <= line, "a meal and a night in hand: no scavenging ({after})");
        }
    }
}

/// Parity (the law1 actor share): with the table's `p_steal_broke` the
/// off-screen hour's theft roll is the broke adult's alone, as a Full agent
/// with a meal's coins all but never steals (seed 2000, all-Full: 1 theft
/// of ~240 by a thief who could buy); the rows' `p_steal` is not read. A
/// table without it keeps the rows' roll for everyone.
#[test]
fn test_statistical_theft_roll_is_the_broke_adults() {
    use citysim::{EventKind, Wallet};
    for (learned, broke) in [(true, true), (true, false), (false, false)] {
        let (mut w, id) = no_net_stat_agent();
        w.comp_mut::<Needs>(id).expect("needs").hunger = 1.0;
        let price = citysim::systems::life::meal_price(&w, id);
        w.comp_mut::<Wallet>(id).expect("wallet").coins = if broke { price - 1 } else { 50 * price };
        if let Some(t) = w.stat_table.as_mut() {
            for r in &mut t.rows {
                r.p_steal = 1.0;
            }
            t.p_steal_broke = learned.then_some([1.0; citysim::STAT_ROWS]);
        }
        let start = w.events.back().map_or(0, |e| e.id + 1);
        stat_hour(&mut w);
        let stole =
            w.events.iter().any(|e| e.id >= start && e.kind == EventKind::Theft && e.actors.first() == Some(&id));
        assert_eq!(stole, broke || !learned, "learned {learned}, broke {broke}");
    }
}

/// Flip readiness: with no net a hungry Statistical agent who cannot buy
/// steals by its lawfulness bucket's `p_desperate` (the lawful go hungry, as
/// the Full planner's theft cost has them), and a theft takes a Full Market
/// theft's two units (one eaten at once).
#[test]
fn test_no_net_need_theft_follows_lawfulness() {
    use citysim::{EventKind, Inventory, Personality};
    for lawful in [false, true] {
        let (mut w, id) = no_net_stat_agent();
        w.comp_mut::<Personality>(id).expect("personality").lawfulness = if lawful { 0.9 } else { 0.1 };
        {
            let n = w.comp_mut::<Needs>(id).expect("needs");
            n.hunger = 0.2;
            n.starving_since = None;
        }
        if let Some(t) = w.stat_table.as_mut() {
            t.p_desperate = Some([1.0, 0.0, 0.0]);
        }
        let start = w.events.back().map_or(0, |e| e.id + 1);
        stat_hour(&mut w);
        let stole =
            w.events.iter().any(|e| e.id >= start && e.kind == EventKind::Theft && e.actors.first() == Some(&id));
        if lawful {
            assert!(!stole, "a lawful agent goes hungry rather than steal");
            assert_eq!(w.comp::<Inventory>(id).map(|i| i.food), Some(0));
        } else {
            assert!(stole, "the lawless steal the meal they cannot buy");
            assert!(w.comp::<Needs>(id).is_some_and(|n| n.hunger > 0.2), "and eat it");
            assert_eq!(w.comp::<Inventory>(id).map(|i| (i.food, i.stolen_food)), Some((1, 1)), "two taken, one eaten");
        }
    }
}

/// Flip readiness (night porter): with wages on, a worker promoted on its
/// own shift stands at its workplace whatever the phase (a night shift
/// starts at the Hotel, not across town), and a homeless worker off shift
/// at night stands at its workplace rather than the cheapest Market.
#[test]
fn test_night_shift_worker_promoted_at_the_workplace() {
    use citysim::{Building, Job, Position, TilePos};
    let cfg = Config::load();
    let mut w = World::new(42, cfg);
    let role = w.config.trade_role("night_porter").expect("the Night Porter trade");
    // Day 1 (a workday), 00:30: the night shift is on.
    w.run_ticks(TICKS_PER_DAY + 30);
    let p = w
        .workers(role)
        .iter()
        .copied()
        .find(|&a| {
            !w.has::<Sentence>(a)
                && w.comp::<Job>(a).is_some_and(|j| {
                    j.on_shift(w.tick_of_day()) && citysim::exec::routine::workday_of(&w, a, j, j.shift_key_at(w.tick))
                })
        })
        .expect("a porter on shift");
    let hotel = w.comp::<Job>(p).and_then(|j| j.employer).expect("its Hotel");
    let door = w.comp::<Building>(hotel).expect("hotel").door;
    let far = TilePos { x: 3, y: 3 };
    for homeless in [false, true] {
        if homeless {
            w.set_home(p, None);
        }
        lod::set_lod(&mut w, p, Lod::Statistical);
        w.leave_building(p);
        w.comp_mut::<Position>(p).expect("pos").tile = far;
        lod::set_lod(&mut w, p, Lod::Full);
        assert_eq!(w.comp::<Position>(p).map(|x| x.tile), Some(door), "homeless {homeless}: at the Hotel on shift");
    }
    // Off shift at night (a day worker's hours), a homeless worker sleeps near the work.
    if let Some(j) = w.comp_mut::<Job>(p) {
        j.shifts = vec![(600, 1080)];
    }
    lod::set_lod(&mut w, p, Lod::Statistical);
    assert_eq!(w.comp::<Position>(p).map(|x| x.tile), Some(door), "a homeless worker's night at the workplace");
}

/// Flip readiness (review): `p_desperate` is learned from adults, so a
/// Statistical minor keeps the M10 rule: hungry is no theft, starving is.
#[test]
fn test_no_net_child_keeps_the_starving_theft_rule() {
    use citysim::{EventKind, Inventory, Wallet};
    for starving in [false, true] {
        let cfg = Config::load();
        let mut w = World::new(42, cfg);
        w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
        let id = w
            .tier(Lod::Statistical)
            .iter()
            .copied()
            .find(|&a| citysim::systems::demography::is_adult(&w, a) && !w.has::<Sentence>(a))
            .expect("a Statistical agent");
        // Under age (a minor with a Brain: the off-screen eat path's child).
        w.comp_mut::<citysim::Identity>(id).expect("identity").age_days =
            citysim::systems::demography::ADULT_AGE_DAYS - 1;
        assert!(!citysim::systems::demography::is_adult(&w, id));
        w.set_home(id, None);
        w.comp_mut::<Inventory>(id).expect("inv").food = 0;
        w.comp_mut::<Inventory>(id).expect("inv").stolen_food = 0;
        if let Some(x) = w.comp_mut::<Wallet>(id) {
            x.coins = 0;
        }
        if let Some(t) = w.stat_table.as_mut() {
            t.p_scavenge = Some([0.0; citysim::STAT_ROWS]);
            t.p_beg = Some([0.0; citysim::STAT_ROWS]);
            t.p_desperate = Some([1.0; 3]);
        }
        {
            let n = w.comp_mut::<Needs>(id).expect("needs");
            n.hunger = if starving { 0.0 } else { 0.2 };
            n.starving_since = None;
        }
        let start = w.events.back().map_or(0, |e| e.id + 1);
        stat_hour(&mut w);
        let stole =
            w.events.iter().any(|e| e.id >= start && e.kind == EventKind::Theft && e.actors.first() == Some(&id));
        assert_eq!(stole, starving, "starving {starving}: a child steals only when starving");
    }
}

/// Hunger as sickness P5h (a harness change): the all-Full calibration and
/// parity cities scale the planner budgets by n / 500 (never below the
/// shipped values), so a 2,000-agent Full city is not a planner-starved one.
#[test]
fn test_calibration_city_scales_the_planner_budget() {
    let base = Config::load();
    let (p, e) = (base.brain.plan_budget_per_tick, base.brain.plan_expansion_budget_per_tick);
    let at = |n: u32| {
        let c = Config::load().calibration_city(n);
        (c.brain.plan_budget_per_tick, c.brain.plan_expansion_budget_per_tick)
    };
    assert_eq!(at(500), (p, e));
    assert_eq!(at(250), (p, e), "never below the shipped budget");
    assert_eq!(at(2000), (4 * p, 4 * e));
}

/// Hunger as sickness P5h, a printed finding (not an assert): the planner
/// queue in the shipped mixed-LOD city on seed 42 (does a busy on-screen
/// crowd starve the planner in the real game too?). Max and mean queue
/// length sampled hourly, and the stale drops, over 3 days.
#[test]
fn finding_planner_queue_shipped_city() {
    let mut w = World::new(42, Config::load());
    let mut lens = Vec::new();
    let mut full_max = 0;
    for _ in 0..(3 * 24) {
        w.run_ticks(TICKS_PER_HOUR);
        lens.push(w.plan_queue.len());
        full_max = full_max.max(counts(&w).0);
    }
    let max = lens.iter().copied().max().unwrap_or(0);
    let mean = lens.iter().sum::<usize>() as f64 / lens.len().max(1) as f64;
    eprintln!(
        "FINDING planner queue (shipped, seed 42, 3 days): budget {}/tick; queue max {max} mean {mean:.1}; stale drops {}; Full agents max {full_max}",
        w.config.brain.plan_budget_per_tick, w.plan_stale_drops
    );
}

/// Diet parity (hunger fix item 11's off-screen side): an employed
/// Statistical agent off shift, with a meal's coins and an empty household
/// pantry, stocks the pantry in its hour (the Full worker's grocery run);
/// with item 11 off it does not.
#[test]
fn test_statistical_worker_off_shift_stocks_the_pantry() {
    use citysim::{Household, Inventory, Job, Needs, Wallet};
    for on in [true, false] {
        let mut cfg = Config::load();
        cfg.life.hf_shift_meals = on;
        let mut w = World::new(42, cfg);
        w.run_ticks(TICKS_PER_DAY + 20 * TICKS_PER_HOUR);
        let tod = w.tick_of_day();
        let id = w
            .tier(Lod::Statistical)
            .iter()
            .copied()
            .find(|&a| {
                citysim::systems::demography::is_adult(&w, a)
                    && !w.has::<Sentence>(a)
                    && w.comp::<Job>(a).is_some_and(|j| j.employer.is_some() && !j.on_shift(tod))
                    && w.comp::<Household>(a).is_some_and(|h| h.home.is_some())
            })
            .expect("an employed Statistical adult off shift");
        let home = w.comp::<Household>(id).and_then(|h| h.home).unwrap();
        for r in w.residents_of(home).to_vec() {
            if let Some(i) = w.comp_mut::<Inventory>(r) {
                i.food = 0;
                i.synth = 0;
                i.stolen_food = 0;
            }
            if let Some(n) = w.comp_mut::<Needs>(r) {
                n.hunger = 0.95;
            }
        }
        if let Some(b) = w.comp_mut::<citysim::Building>(home) {
            b.stock_food = 0;
            b.synth_food = 0;
        }
        w.comp_mut::<Wallet>(id).expect("wallet").coins = 60;
        stat_hour(&mut w);
        let stock = w.comp::<citysim::Building>(home).map_or(0, |b| b.stock_food);
        eprintln!("item 11 {on}: pantry after the hour {stock}");
        assert_eq!(stock > 0, on, "the off-shift worker's pantry run (item 11 {on})");
    }
}

/// Diet parity: a meal at hand (carried, or in the Home pantry) is eaten
/// before a bin is: `bins::dive_target` has none for an agent holding one,
/// in both tiers.
#[test]
fn test_no_bin_with_a_meal_at_hand() {
    use citysim::{Household, Inventory, Needs, Wallet};
    let cfg = Config::load();
    let mut w = World::new(42, cfg);
    w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
    // A housed broke adult, starving, with a bin in reach once its food is gone.
    let found = w.citizens().into_iter().find_map(|a| {
        if !citysim::systems::demography::is_adult(&w, a) || w.has::<Sentence>(a) {
            return None;
        }
        let home = w.comp::<Household>(a).and_then(|h| h.home)?;
        let mut c = w.clone();
        c.comp_mut::<Wallet>(a)?.coins = 0;
        c.comp_mut::<Needs>(a)?.hunger = 0.05;
        if let Some(i) = c.comp_mut::<Inventory>(a) {
            i.food = 0;
            i.synth = 0;
            i.stolen_food = 0;
        }
        c.comp_mut::<citysim::Building>(home)?.stock_food = 0;
        citysim::systems::bins::dive_target(&c, a).map(|_| (a, home, c))
    });
    let Some((a, home, mut c)) = found else { panic!("no broke adult with a bin in reach") };
    c.comp_mut::<citysim::Building>(home).unwrap().stock_food = 2;
    assert!(citysim::systems::bins::dive_target(&c, a).is_none(), "a bin with a pantry meal at home");
    c.comp_mut::<citysim::Building>(home).unwrap().stock_food = 0;
    c.comp_mut::<Inventory>(a).unwrap().food = 1;
    assert!(citysim::systems::bins::dive_target(&c, a).is_none(), "a bin with a meal in hand");
}

/// Scale S1: `crew_above`, a Statistical worker's co-workers above it for
/// the hour's co-work. `crew` 0 and a group of at most `crew + 1` are every
/// co-worker above it (the pre-S1 rule); a bigger group is the `crew / 2`
/// nearest on each side in id order, wrapping, so every member drives at
/// most `crew` pairs in all and each pair is driven once (membership is
/// symmetric); other roles at the employer are passed over.
#[test]
fn test_crew_above_bounds_coworker_pairs() {
    let ids = |r: std::ops::Range<u32>| -> Vec<citysim::EntityId> {
        r.map(|index| citysim::EntityId { index, generation: 0 }).collect()
    };
    let all = |_: citysim::EntityId| true;
    let small = ids(0..10);
    for crew in [0, 9, 16] {
        assert_eq!(lod::crew_above(&small, small[3], crew, all), small[4..].to_vec(), "crew {crew}");
    }
    assert_eq!(lod::crew_above(&small, small[3], 4, all), vec![small[4], small[5]]);
    // Wrapping: the last member's crew holds the first ones, driven by them.
    assert_eq!(lod::crew_above(&small, small[9], 4, all), Vec::<citysim::EntityId>::new());
    assert_eq!(lod::crew_above(&small, small[0], 4, all), vec![small[1], small[2], small[8], small[9]]);
    let big = ids(0..100);
    for crew in [2usize, 8, 32] {
        let mut pairs = std::collections::BTreeSet::new();
        for &a in &big {
            for b in lod::crew_above(&big, a, crew, all) {
                assert!(b > a);
                assert!(pairs.insert((a, b)), "pair driven twice");
            }
        }
        assert_eq!(pairs.len(), big.len() * crew / 2, "crew {crew}: each member in `crew` pairs");
    }
    // Another role in between is skipped, not counted.
    let even = |e: citysim::EntityId| e.index.is_multiple_of(2);
    assert_eq!(lod::crew_above(&big, big[10], 4, even), vec![big[12], big[14]]);
}

/// Scale S1: `[lod] coworker_crew` bounds the hour's co-work: with a crew of
/// 2, the co-workers of a big group are tied to their two neighbours, not to
/// the whole group (the pre-S1 rule, `coworker_crew` 0, ties every pair).
#[test]
fn test_coworker_crew_bounds_the_coworker_edges() {
    let tied = |crew: usize| -> (usize, usize) {
        let mut cfg = Config::load();
        cfg.lod.coworker_crew = crew;
        let mut w = World::new(42, cfg);
        w.run_ticks(2 * TICKS_PER_DAY);
        // The staff of every workplace's biggest role, at least 20 strong.
        let (mut tied, mut groups) = (0, 0);
        for staff in w.employers.values() {
            let mut by_role: std::collections::BTreeMap<String, Vec<citysim::EntityId>> = Default::default();
            for &c in staff {
                if let Some(j) = w.comp::<citysim::Job>(c) {
                    by_role.entry(j.role.label().to_string()).or_default().push(c);
                }
            }
            for g in by_role.values().filter(|g| g.len() >= 20) {
                groups += 1;
                for (i, &a) in g.iter().enumerate() {
                    tied += g[i + 1..].iter().filter(|&&b| w.edge(a, b).is_some()).count();
                }
            }
        }
        (tied, groups)
    };
    let (all, groups) = tied(0);
    let (crew, _) = tied(2);
    eprintln!("co-worker pairs tied in {groups} groups of 20+: every pair {all}, crew 2 {crew}");
    assert!(groups > 0, "no workplace has 20 of a role");
    assert!(crew * 3 < all, "crew 2 tied {crew} pairs, every co-worker {all}");
}
