//! Life pass L2 phase 1 (docs/LIFE_L2.md § 1, § 10; plan phase 1): the
//! seven new kinds, staffing, the Fab and the scrap chain, the budget band,
//! Sweep and the `--l2-off` identity.

use citysim::systems::{budget, districts, founding, jobs, ownership};
use citysim::{Building, BuildingKind, Config, EntityId, Good, Job, Role, Wallet, World, TICKS_PER_DAY};

fn kind_of(w: &World, b: EntityId) -> BuildingKind {
    w.comp::<Building>(b).expect("building").kind
}

fn standing(w: &World, kind: BuildingKind) -> Vec<EntityId> {
    w.buildings_of_kind(kind)
        .iter()
        .copied()
        .filter(|&b| w.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .collect()
}

/// Every new kind stands at seed; by day 7 every role has worked a paid
/// shift; the Fab holds Parts.
#[test]
fn test_each_new_kind_seeds_and_pays_a_shift() {
    let mut w = World::new(42, Config::load());
    assert!(jobs::on(&w));
    for kind in [
        BuildingKind::Club,
        BuildingKind::Arcade,
        BuildingKind::NoodleBar,
        BuildingKind::FightPit,
        BuildingKind::Den,
        BuildingKind::Lounge,
        BuildingKind::Fab,
    ] {
        let list = standing(&w, kind);
        assert!(!list.is_empty(), "a {} stands at seed", kind.label());
        if kind.is_leisure() {
            assert!(list.iter().all(|&b| w.comp::<Building>(b).is_some_and(|bd| bd.venue.is_some())), "a venue");
        }
    }
    w.run_ticks(7 * TICKS_PER_DAY);
    for role in [Role::Host, Role::Attendant, Role::Cook, Role::Fighter, Role::Croupier, Role::Concierge, Role::Fabber]
    {
        // A shift worked and nothing owed (L1: paid at the shift's end, by
        // `stat_work` for a Statistical worker).
        let paid = w
            .workers(role)
            .iter()
            .any(|&a| w.comp::<Job>(a).is_some_and(|j| j.last_shift_day.is_some() && j.days_unpaid == 0));
        assert!(paid, "a {} worked a paid shift by day 7", role.label());
    }
    let parts: u32 = standing(&w, BuildingKind::Fab).iter().map(|&f| w.stock(f, Good::Parts)).sum();
    let made: u32 = w.stats.history.iter().map(|r| r.living.fab_parts).sum();
    assert!(made > 0 && parts + made > 0, "the Fabs made Parts: made {made}, held {parts}");
}

/// § 1: at 130 coins only the NoodleBar (120) is affordable, and it is the
/// kind the founder would register.
#[test]
fn test_register_chooses_noodle_bar_at_120() {
    let w = World::new(42, Config::load());
    assert_eq!(founding::choose_kind_for(&w, 130, false), Some(BuildingKind::NoodleBar));
    // With L2 off nothing is affordable at 130 (the floor is the Bar's).
    let off = World::new(42, Config::load().living_off());
    assert_eq!(founding::choose_kind_for(&off, 130, false), None);
}

/// L6: a derelict Block refitted as a Den at `refit_frac` of its cost;
/// coins are conserved (the founder pays the Treasury).
#[test]
fn test_refit_turns_derelict_into_den_at_half_cost() {
    let mut w = World::new(42, Config::load());
    let d = citysim::systems::street::derelicts(&w)
        .into_iter()
        .find(|&b| {
            kind_of(&w, b) == BuildingKind::Home
                && founding::tier_ok(BuildingKind::Den, w.comp::<Building>(b).expect("b").tier)
        })
        .expect("a derelict Block a Den may take");
    let founder = w
        .citizens()
        .into_iter()
        .find(|&a| w.has::<Wallet>(a) && citysim::systems::demography::is_adult(&w, a))
        .expect("an adult");
    w.comp_mut::<Wallet>(founder).expect("wallet").coins = 1000;
    let before = ownership::total_coins(&w);
    let treasury = w.treasury().expect("treasury").coins;
    founding::refit(&mut w, d, BuildingKind::Den, Some(founder)).expect("refit");
    let half = (w.config.jobs.refit_frac * w.config.corps.found_cost.den as f32).round() as i64;
    assert_eq!(w.comp::<Wallet>(founder).expect("wallet").coins, 1000 - half);
    assert_eq!(w.treasury().expect("treasury").coins, treasury + half);
    assert_eq!(ownership::total_coins(&w), before, "money conserved");
    let bd = w.comp::<Building>(d).expect("den");
    assert_eq!(bd.kind, BuildingKind::Den);
    assert!(!bd.derelict && bd.venue.is_some() && bd.owner == Some(founder));
    assert!(w.buildings_of_kind(BuildingKind::Den).contains(&d));
    assert!(!w.buildings_of_kind(BuildingKind::Home).contains(&d));
    assert!(w.vacancies.get(&d).is_some_and(|v| v.iter().all(|&r| r == Role::Croupier) && !v.is_empty()));
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::Refit));
}

fn market_staffing(w: &World) -> Vec<usize> {
    standing(w, BuildingKind::Market)
        .into_iter()
        .map(|m| {
            let staff = ownership::staff_at(w, m)
                .into_iter()
                .filter(|&a| w.comp::<Job>(a).is_some_and(|j| j.role == Role::Clerk))
                .count();
            let open = w.vacancies.get(&m).map_or(0, |v| v.iter().filter(|&&r| r == Role::Clerk).count());
            staff + open
        })
        .collect()
}

/// L3: with jobs on, the first midnight posts every Market's deficit to 12
/// (a hunkering corp's to 6); with L2 off a Market keeps its 4.
#[test]
fn test_market_staff_tops_up_to_12_only_with_jobs_on() {
    let mut on = World::new(42, Config::load());
    on.run_ticks(TICKS_PER_DAY + 1);
    let hunker = |w: &World, m: EntityId| {
        w.owner_of(m).and_then(|o| w.comp::<citysim::Corp>(o)).is_some_and(|c| c.order == citysim::CorpOrder::Hunker)
    };
    let staffed: Vec<usize> = market_staffing(&on);
    let markets = standing(&on, BuildingKind::Market);
    // A hunkering corp's Market staffs to half (Hunker's floor).
    assert!(
        markets.iter().zip(&staffed).all(|(&m, &n)| n >= if hunker(&on, m) { 6 } else { 12 }),
        "every Market staffs up to 12 (6 under Hunker): {staffed:?}"
    );
    assert_eq!(jobs::full_staff(&on, BuildingKind::Market), 12);
    assert_eq!(jobs::full_staff(&on, BuildingKind::Bar), 5);
    let mut off = World::new(42, Config::load().living_off());
    off.run_ticks(TICKS_PER_DAY + 1);
    assert!(market_staffing(&off).iter().all(|&n| n <= 4), "off: {:?}", market_staffing(&off));
    assert_eq!(jobs::full_staff(&off, BuildingKind::Market), 4);
}

/// L9: a Fab accrues Parts from labour; the parts market sources a corp's
/// own Fab before the Recycler, at no coin to itself.
#[test]
fn test_fab_accrues_and_parts_market_buys_fab_before_recycler() {
    let mut w = World::new(42, Config::load());
    let fab = standing(&w, BuildingKind::Fab).into_iter().next().expect("a Fab");
    let corp = w.owner_of(fab).expect("a corp's Fab");
    let worker = w.citizens()[0];
    jobs::accrue_fab_work(&mut w, worker, fab, 60 * 60);
    let made = w.stock(fab, Good::Parts);
    assert!(made >= 20, "an hour-long day of Fab work made Parts: {made}");
    // A buyer of the same corp below the floor; the Recycler holds Parts too.
    let buyer = [BuildingKind::Garage, BuildingKind::Clinic]
        .into_iter()
        .flat_map(|k| standing(&w, k))
        .find(|&b| w.owner_of(b) == Some(corp));
    let buyer = match buyer {
        Some(b) => b,
        None => {
            let g = standing(&w, BuildingKind::Garage).into_iter().next().expect("a Garage");
            ownership::transfer_building(&mut w, g, Some(corp));
            g
        }
    };
    let others: Vec<EntityId> = [BuildingKind::Garage, BuildingKind::Clinic]
        .into_iter()
        .flat_map(|k| standing(&w, k))
        .filter(|&b| b != buyer)
        .collect();
    for b in others {
        let cap = w.goods_cap(b, Good::Parts);
        w.add_stock(b, Good::Parts, cap);
    }
    for g in w.gangs() {
        if let Some(h) = w.hideout_of(g) {
            let n = w.stock(h, Good::Parts);
            w.take_stock(h, Good::Parts, n);
        }
    }
    let n = w.stock(buyer, Good::Parts);
    w.take_stock(buyer, Good::Parts, n);
    let recycler = w.building_of_kind(BuildingKind::Cemetery).expect("Recycler");
    w.add_stock(recycler, Good::Parts, 50);
    let purse = w.purse(Some(corp));
    citysim::systems::assets::parts_market(&mut w);
    let got = w.stock(buyer, Good::Parts);
    assert!(got > 0, "the buyer restocked");
    assert_eq!(w.stock(fab, Good::Parts), made - got, "from its own Fab");
    assert_eq!(w.stock(recycler, Good::Parts), 50, "not from the Recycler");
    assert_eq!(w.purse(Some(corp)), purse, "its own Fab's Parts cost nothing");
}

/// L9: midnight turns `scrap_per_part` scrap into one Recycler Part, the
/// remainder kept.
#[test]
fn test_scrap_becomes_recycler_part_at_scrap_per_part() {
    let mut w = World::new(42, Config::load());
    let recycler = w.building_of_kind(BuildingKind::Cemetery).expect("Recycler");
    let per = w.config.jobs.scrap_per_part;
    let before = w.stock(recycler, Good::Parts);
    w.scrap = 2 * per + 1;
    jobs::daily(&mut w);
    assert_eq!(w.stock(recycler, Good::Parts), before + 2);
    assert_eq!(w.scrap, 1);
    // With L2 off a find adds no scrap.
    let mut off = World::new(42, Config::load().living_off());
    jobs::add_scrap(&mut off);
    assert_eq!(off.scrap, 0);
}

/// L10: above `hi` for three days the band posts `works_step` public works;
/// once they stand full upkeep falls; below `lo` for three days upkeep rises
/// and the newest public-works hire is laid off.
#[test]
fn test_band_posts_works_above_hi_and_lifts_upkeep_below_lo() {
    let mut w = World::new(42, Config::load());
    let recycler = w.building_of_kind(BuildingKind::Cemetery).expect("Recycler");
    let hi = w.config.budget.band[1];
    let lo = w.config.budget.band[0];
    w.treasury_mut().expect("treasury").coins = hi + 10_000;
    budget::daily(&mut w);
    budget::daily(&mut w);
    assert_eq!(w.works_vacancies, 0, "hysteresis: two days are not enough");
    budget::daily(&mut w);
    let step = w.config.budget.works_step;
    assert_eq!(w.works_vacancies, step);
    let open = w.vacancies.get(&recycler).map_or(0, |v| v.iter().filter(|&&r| r == Role::Sanitation).count());
    assert!(open >= usize::from(step));
    assert!(w.events.iter().any(|e| e.kind == citysim::EventKind::WorksPosted));
    // Hire into the works: the roster fills, oldest first.
    let jobless: Vec<EntityId> = w
        .citizens()
        .into_iter()
        .filter(|&a| !w.has::<Job>(a) && citysim::systems::demography::is_adult(&w, a))
        .take(usize::from(w.config.budget.works_max))
        .collect();
    for &a in &jobless {
        citysim::systems::demography::hire(&mut w, a, recycler, Role::Sanitation);
        if w.works_vacancies == 0 {
            w.works_vacancies = 1;
        }
        budget::note_hire(&mut w, a, recycler, Role::Sanitation);
    }
    w.works_vacancies = 0;
    let mult = w.budget.upkeep_mult;
    // Fix round: a step fires once per `band_hold_days` (days 4, 5: none; day 6: the cut).
    budget::daily(&mut w);
    budget::daily(&mut w);
    assert_eq!(w.budget.upkeep_mult, mult, "no step between holds");
    budget::daily(&mut w);
    assert!(w.budget.upkeep_mult < mult, "the works stand full: upkeep falls ({mult} -> {})", w.budget.upkeep_mult);
    // The forward guard: under hi + 7 x yesterday's dole, no cut.
    let mut g = w.stats.history.back().cloned().unwrap_or_else(|| citysim::DayRow::new(0));
    g.flow_dole = 10_000;
    w.stats.history.push_back(g);
    let held_mult = w.budget.upkeep_mult;
    for _ in 0..3 {
        budget::daily(&mut w);
    }
    assert_eq!(w.budget.upkeep_mult, held_mult, "a week of dole above hi is not there: no cut");
    // Below the band.
    w.treasury_mut().expect("treasury").coins = lo - 10_000;
    let newest = *w.jobs_book.works.last().expect("works");
    let before = w.jobs_book.works.len();
    let low = w.budget.upkeep_mult;
    budget::daily(&mut w);
    budget::daily(&mut w);
    assert!(w.has::<Job>(newest), "hysteresis below too");
    budget::daily(&mut w);
    let step = w.config.budget.upkeep_step;
    assert!(w.budget.upkeep_mult >= (low + 2.0 * step).min(1.0) - 1e-4, "upkeep rises by twice the step");
    assert!(!w.has::<Job>(newest), "the newest public-works hire is laid off");
    assert!(!w.jobs_book.works.contains(&newest));
    assert_eq!(w.jobs_book.works.len(), before - usize::from(w.config.budget.works_step), "works_step at once");
}

/// L8: Sweep cleans the beat's dirtiest streets, and the D23 midnight
/// credit skips that sweeper (no double count).
#[test]
fn test_sweep_cleans_beat_and_skips_d23_credit() {
    let mut w = World::new(42, Config::load());
    assert!(jobs::sweep_on(&w));
    w.run_ticks(TICKS_PER_DAY + 1);
    let s = *w.workers(Role::Sanitation).first().expect("a sweeper");
    // The only sweeper: the others go.
    for o in w.workers(Role::Sanitation).to_vec() {
        if o != s {
            w.remove::<Job>(o);
        }
    }
    let d = jobs::beat_of(&w, s).expect("a beat");
    let streets = w.districts[d.index()].streets.clone();
    for &i in &streets {
        w.litter[i as usize] = 200;
    }
    let total = |w: &World| -> u64 { streets.iter().map(|&i| u64::from(w.litter[i as usize])).sum() };
    let before = total(&w);
    jobs::sweep_done(&mut w, s, 8 * 60);
    let swept = before - total(&w);
    assert_eq!(swept, u64::from(8 * w.config.jobs.sweep_per_hour), "eight hours on the beat");
    assert!(w.jobs_book.swept.contains(&s));
    // At the next midnight the D23 credit skips the sweeper who swept.
    let yesterday = w.comp::<Job>(s).expect("job").shift_key_at(w.tick);
    w.comp_mut::<Job>(s).expect("job").last_shift_day = Some(yesterday);
    w.tick = (w.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
    w.sweep_beats.insert(s, d);
    let mid = total(&w);
    districts::sanitation(&mut w);
    assert_eq!(total(&w), mid, "no second credit for the sweeper who swept");
    assert!(w.jobs_book.swept.is_empty(), "the day's list is cleared");
}

/// L32: an L2 build with `living_off` has no venues and runs its first day
/// as a config without any L2 key (the serde defaults are all off).
#[test]
fn test_l2_off_world_has_no_venues_and_same_first_day() {
    let text = std::fs::read_to_string(Config::load().asset("config.toml")).expect("config.toml");
    // The config before any L2 key existed: the L2 tail cut off.
    let cut = text.find("# Life pass L2").expect("the L2 block");
    let mut old: Config = toml::from_str(&text[..cut]).expect("the pre-L2 config parses");
    old.assets_dir = Config::load().assets_dir;
    assert!(!old.living.enabled && !old.jobs.enabled && !old.budget.enabled);
    let mut a = World::new(42, Config::load().living_off());
    let mut b = World::new(42, old);
    for kind in [BuildingKind::Club, BuildingKind::NoodleBar, BuildingKind::Fab] {
        assert!(a.buildings_of_kind(kind).is_empty() && b.buildings_of_kind(kind).is_empty());
    }
    a.run_ticks(TICKS_PER_DAY);
    b.run_ticks(TICKS_PER_DAY);
    let row = |w: &World| {
        let mut r = w.stats.history.back().expect("a day").clone();
        r.ticks_per_sec = 0.0;
        r.csv_row()
    };
    assert_eq!(row(&a), row(&b), "the same first day");
    // The worlds match too, once their configs are made equal.
    b.config = a.config.clone();
    assert_eq!(citysim::save::to_ron(&a), citysim::save::to_ron(&b));
    assert!(a.outside.is_empty() && a.scrap == 0 && a.budget.is_default());
}

/// L2 fix round: a fresh hire draws the dole on its hire day, and not after
/// the job's first wage.
#[test]
fn test_new_hire_draws_dole_until_first_wage() {
    use citysim::systems::economy;
    let mut w = World::new(42, Config::load());
    let recycler = w.building_of_kind(BuildingKind::Cemetery).expect("Recycler");
    let a = w
        .citizens()
        .into_iter()
        .find(|&a| !w.has::<Job>(a) && w.has::<citysim::Brain>(a) && citysim::systems::demography::is_adult(&w, a))
        .expect("a jobless adult");
    citysim::systems::demography::hire(&mut w, a, recycler, Role::Sanitation);
    assert!(!w.comp::<Job>(a).expect("job").paid_once);
    let coins = w.comp::<Wallet>(a).expect("wallet").coins;
    assert!(economy::collect_dole(&mut w, a), "the hire draws the dole on its hire day");
    assert!(w.comp::<Wallet>(a).expect("wallet").coins > coins);
    // The first wage.
    w.comp_mut::<Job>(a).expect("job").days_unpaid = 1;
    assert!(economy::collect_wage(&mut w, a) > 0);
    assert!(w.comp::<Job>(a).expect("job").paid_once);
    w.comp_mut::<citysim::Brain>(a).expect("brain").last_dole_day = None;
    assert!(!economy::collect_dole(&mut w, a), "no dole after the first wage");
    // With L2 off a hire is paid_once from the start: never on the dole.
    let mut off = World::new(42, Config::load().living_off());
    let r = off.building_of_kind(BuildingKind::Cemetery).expect("Recycler");
    let b = off
        .citizens()
        .into_iter()
        .find(|&b| {
            !off.has::<Job>(b) && off.has::<citysim::Brain>(b) && citysim::systems::demography::is_adult(&off, b)
        })
        .expect("a jobless adult");
    citysim::systems::demography::hire(&mut off, b, r, Role::Sanitation);
    assert!(off.comp::<Job>(b).expect("job").paid_once);
    assert!(!economy::collect_dole(&mut off, b));
}

/// Review fix: the planner reads `economy::dole_eligible`, so with
/// `dole_in_place` off a fresh unpaid hire, broke at the Hall, plans
/// `CollectDole` (before, only `life::dole_in_place` paid it).
#[test]
fn test_unpaid_hire_plans_collect_dole() {
    use citysim::goap::planner::{self, Limits};
    use citysim::{ActionKind, Key, PlanCtx, WorldState};
    let mut cfg = Config::load();
    cfg.life.dole_in_place = false;
    let mut w = World::new(42, cfg);
    let recycler = w.building_of_kind(BuildingKind::Cemetery).expect("Recycler");
    let hall = w.building_of_kind(BuildingKind::Hall).expect("Hall");
    let a = w
        .citizens()
        .into_iter()
        .find(|&a| !w.has::<Job>(a) && w.has::<citysim::Brain>(a) && citysim::systems::demography::is_adult(&w, a))
        .expect("a jobless adult");
    citysim::systems::demography::hire(&mut w, a, recycler, Role::Sanitation);
    assert!(!w.comp::<Job>(a).expect("job").paid_once, "a fresh hire is unpaid");
    w.comp_mut::<Wallet>(a).expect("wallet").coins = 0;
    w.leave_building(a);
    w.enter_building(a, hall);
    let ctx = PlanCtx::build(&w, a, None);
    assert!(ctx.dole_available, "the dole is due to an unpaid hire");
    let start = WorldState::observe(&w, a, None);
    let limits = Limits { max_expansions: w.config.brain.plan_max_expansions, max_len: w.config.brain.plan_max_len };
    let found = planner::plan(&ctx, start, &vec![(Key::HasSavings, true)], limits).expect("a plan");
    assert!(found.steps.contains(&ActionKind::CollectDole), "plan {:?}", found.steps);
}
