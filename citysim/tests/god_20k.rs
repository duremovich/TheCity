//! The 20k city's god scenario (docs/LIVES_AT_WORK.md § 10, the gate):
//! `god_cut_the_precinct`. The district Stations (§ 5.2) stand only on the
//! 20,000 island (the 2k map's districts sit inside `station_min_tiles`),
//! so its reaction needs the big city: two 20k runs in parallel, ~5 min,
//! the milestone tier (`tools/test.py milestone`), not the merge tier's
//! god step. Class: the law's reach (TESTING.md's god class table).
//!
//! Run `cargo test --release -p citysim --test god_20k -- --ignored --nocapture`.

use citysim::{BuildingKind, Config, EntityId, Job, PlayerCommand, Role, World, TICKS_PER_DAY};

const SEED: u64 = 42;
/// The shock's day and the run's end: 20 days on, the ten days the guards
/// are held and ten after (40 days, not the 2k tier's 60: two 20k cities
/// for 60 days would take the milestone tier past its budget).
const SHOCK_DAY: u64 = 20;
const END_DAY: u64 = 40;

/// `core_20k`'s city with the Stations on or off (`[law] stations`).
fn city(stations: bool) -> World {
    let mut c = Config::load().scaled_to(20_000);
    c.world.map = "gen:20000".into();
    c.ambient.flats_per_slot = 50.0;
    c.law.stations = stations;
    World::new(SEED, c)
}

/// One run's days: arrests, Station bookings, transfers; and the guards jailed.
struct Cut {
    stations: bool,
    arrests: Vec<u32>,
    bookings: Vec<u32>,
    transfers: Vec<u32>,
    jailed: usize,
    station_guards: usize,
    failed: Vec<String>,
}

impl Cut {
    fn sum(v: &[u32], a: u64, b: u64) -> u32 {
        v[a as usize..b as usize].iter().sum()
    }

    /// Arrests in the ten days of the cut over the ten before it.
    fn kept(&self) -> f64 {
        let before = Self::sum(&self.arrests, SHOCK_DAY - 10, SHOCK_DAY);
        f64::from(Self::sum(&self.arrests, SHOCK_DAY, SHOCK_DAY + 10)) / f64::from(before.max(1))
    }
}

/// The run: on day 20 every guard posted at the Precinct (`Job.employer`
/// the Jail; a Station's guards are posted at their Station) jailed ten days.
fn run(stations: bool) -> Cut {
    let mut w = city(stations);
    let mut c = Cut {
        stations,
        arrests: Vec::new(),
        bookings: Vec::new(),
        transfers: Vec::new(),
        jailed: 0,
        station_guards: 0,
        failed: Vec::new(),
    };
    for day in 0..END_DAY {
        if day == SHOCK_DAY {
            let precinct = w.building_of_kind(BuildingKind::Jail).expect("the Precinct");
            let guards: Vec<EntityId> = w
                .staff_of(precinct)
                .iter()
                .copied()
                .filter(|&g| w.comp::<Job>(g).is_some_and(|j| j.role == Role::Guard))
                .collect();
            c.station_guards =
                w.guards().iter().filter(|&&g| citysim::systems::stations::station_post(&w, g).is_some()).count();
            c.jailed = guards.len();
            for g in guards {
                w.push_command(PlayerCommand::JailAgent { who: g, days: 10 });
            }
        }
        let cursor = w.events.back().map_or(0, |e| e.id + 1);
        w.run_ticks(TICKS_PER_DAY);
        c.failed.extend(
            w.events
                .iter()
                .filter(|e| e.id >= cursor && e.kind == citysim::EventKind::PlayerActionFailed)
                .map(|e| e.text.clone()),
        );
        let row = w.stats.history.back().expect("a finished day");
        let j = row.jobs2.as_ref();
        c.arrests.push(row.arrests);
        c.bookings.push(j.map_or(0, |x| x.station_bookings));
        c.transfers.push(j.map_or(0, |x| x.transfers));
    }
    c
}

/// Lives at work god 5 (§ 10, class: the law's reach): the Precinct's
/// guards jailed ten days on day 20 at 20k. The Stations hold (bookings
/// during the cut) and transfer, and the arrests fall less than in the
/// no-station city under the same cut (where the Precinct's guards are
/// every guard).
#[test]
#[ignore = "tier: milestone"]
fn god_cut_the_precinct() {
    let (on, off) = std::thread::scope(|s| {
        let a = s.spawn(|| run(true));
        let b = s.spawn(|| run(false));
        (a.join().expect("the Stations run"), b.join().expect("the no-station run"))
    });
    for c in [&on, &off] {
        let w = |v: &[u32], a, b| Cut::sum(v, a, b);
        eprintln!(
            "stations {}: {} Precinct guards jailed ({} posted at Stations); arrests 10-19 {}, 20-29 {}, 30-39 {} (kept {:.2}); Station bookings {} / {} / {}, transfers {} / {} / {}",
            c.stations,
            c.jailed,
            c.station_guards,
            w(&c.arrests, 10, 20),
            w(&c.arrests, 20, 30),
            w(&c.arrests, 30, 40),
            c.kept(),
            w(&c.bookings, 10, 20),
            w(&c.bookings, 20, 30),
            w(&c.bookings, 30, 40),
            w(&c.transfers, 10, 20),
            w(&c.transfers, 20, 30),
            w(&c.transfers, 30, 40),
        );
        eprintln!("  arrests by day: {:?}", c.arrests);
    }
    assert!(on.failed.is_empty() && off.failed.is_empty(), "failed: {:?} {:?}", on.failed, off.failed);
    assert!(on.jailed >= 1 && off.jailed >= 1, "no Precinct guard to jail ({} / {})", on.jailed, off.jailed);
    let held = Cut::sum(&on.bookings, SHOCK_DAY, SHOCK_DAY + 10);
    let moved = Cut::sum(&on.transfers, SHOCK_DAY, SHOCK_DAY + 10);
    assert!(
        held > 0 && moved > 0,
        "the Stations did not hold and transfer during the cut ({held} bookings, {moved} transfers)"
    );
    assert!(
        on.kept() > off.kept(),
        "the arrests fell as far with the Stations as without ({:.2} against {:.2})",
        on.kept(),
        off.kept()
    );
}
