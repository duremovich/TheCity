//! The 20k city's god scenario (docs/LIVES_AT_WORK.md § 10, the gate):
//! `god_cut_the_precinct`. The cut is the funded force (`levers.guard_count`
//! halved by `SetGuardCount` for ten days): jailing the Precinct's guards
//! was no cut, since `reconcile_guards` rehired their posts up to the funded
//! count at the next midnight (8c's read). The district Stations (§ 5.2) stand only on the
//! 20,000 island (the 2k map's districts sit inside `station_min_tiles`),
//! so its reaction needs the big city: two 20k runs in parallel, ~5 min,
//! the milestone tier (`tools/test.py milestone`), not the merge tier's
//! god step. Class: the law's reach (TESTING.md's god class table).
//!
//! Run `cargo test --release -p citysim --test god_20k -- --ignored --nocapture`.

use citysim::{Config, PlayerCommand, World, TICKS_PER_DAY};

const SEED: u64 = 42;
/// The shock's day and the run's end: 20 days on, the ten days the force
/// is cut and ten after it is restored (40 days, not the 2k tier's 60: two 20k cities
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

/// One run's days: arrests, Station bookings, transfers, the city's guards
/// (`law_brain::guards`: the corps' private guards are not the lever's);
/// the funded force before the cut and during it, and the State's guards on
/// top of it (`reconcile_guards`' want).
struct Cut {
    stations: bool,
    arrests: Vec<u32>,
    bookings: Vec<u32>,
    transfers: Vec<u32>,
    guards: Vec<u32>,
    funded: u16,
    cut_to: u16,
    state: Vec<u16>,
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

    /// The cut's last five days (the layoffs, five per 2,000 residents a
    /// midnight, given five midnights to land).
    fn cut_days() -> std::ops::Range<usize> {
        (SHOCK_DAY + 5) as usize..(SHOCK_DAY + 10) as usize
    }

    /// The most city guards on a day of the cut's last five.
    fn guards_in_cut(&self) -> u32 {
        self.guards[Self::cut_days()].iter().copied().max().unwrap_or(0)
    }

    /// The most the lever and the State fund on a day of the cut's last
    /// five (`reconcile_guards`' want: `guard_count` + `state_guards`).
    fn want_in_cut(&self) -> u32 {
        self.state[Self::cut_days()].iter().map(|&s| u32::from(self.cut_to) + u32::from(s)).max().unwrap_or(0)
    }
}

/// The run: on day 20 the funded force (`levers.guard_count`) halved by
/// the god's lever (`SetGuardCount`, which pins it against the civic
/// budget), restored on day 30.
fn run(stations: bool) -> Cut {
    let mut w = city(stations);
    let mut c = Cut {
        stations,
        arrests: Vec::new(),
        bookings: Vec::new(),
        transfers: Vec::new(),
        guards: Vec::new(),
        funded: 0,
        cut_to: 0,
        state: Vec::new(),
        station_guards: 0,
        failed: Vec::new(),
    };
    for day in 0..END_DAY {
        if day == SHOCK_DAY {
            c.station_guards =
                w.guards().iter().filter(|&&g| citysim::systems::stations::station_post(&w, g).is_some()).count();
            c.funded = w.levers.guard_count;
            c.cut_to = c.funded / 2;
            w.push_command(PlayerCommand::SetGuardCount(c.cut_to));
        }
        if day == SHOCK_DAY + 10 {
            w.push_command(PlayerCommand::SetGuardCount(c.funded));
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
        c.state.push(w.outside.state_guards);
        let guards = citysim::systems::law_brain::guards(&w).len();
        c.guards.push(u32::try_from(guards).unwrap_or(u32::MAX));
    }
    c
}

/// Lives at work god 5 (§ 10, class: the law's reach): the funded force
/// halved for ten days from day 20 at 20k. Asserted: the cut lands (the
/// city's guards fall to the lever plus the State's guards; 8c's jailing
/// was rehired at midnight) in both cities, and the Stations hold
/// (bookings during the cut) and transfer. Printed: the arrests kept in
/// each city (a FINDING, below).
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
            "stations {}: funded {} cut to {} ({} posted at Stations); State guards day 19 {}, day 29 {}; city guards day 19 {}, cut max {} (funded with the State {}), day 39 {}; arrests 10-19 {}, 20-29 {}, 30-39 {} (kept {:.2}); Station bookings {} / {} / {}, transfers {} / {} / {}",
            c.stations,
            c.funded,
            c.cut_to,
            c.station_guards,
            c.state[(SHOCK_DAY - 1) as usize],
            c.state[(SHOCK_DAY + 9) as usize],
            c.guards[(SHOCK_DAY - 1) as usize],
            c.guards_in_cut(),
            c.want_in_cut(),
            c.guards[(END_DAY - 1) as usize],
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
    for c in [&on, &off] {
        let before = c.guards[(SHOCK_DAY - 1) as usize];
        let want = c.want_in_cut();
        assert!(c.cut_to >= 1 && before > want, "stations {}: no force to cut ({before} against {want})", c.stations);
        assert!(
            c.guards_in_cut() <= want,
            "stations {}: the cut did not land (funded {} cut to {}; city guards day 19 {before}, during the cut up to {} against {want} funded with the State)",
            c.stations,
            c.funded,
            c.cut_to,
            c.guards_in_cut()
        );
    }
    let held = Cut::sum(&on.bookings, SHOCK_DAY, SHOCK_DAY + 10);
    let moved = Cut::sum(&on.transfers, SHOCK_DAY, SHOCK_DAY + 10);
    assert!(
        held > 0 && moved > 0,
        "the Stations did not hold and transfer during the cut ({held} bookings, {moved} transfers)"
    );
    // A FINDING, not an assert (doctrine 1; the 8c fix round): the arrests
    // over the cut against the ten days before are a calibration reading of
    // two noisy ratios, and with the real cut both cities' arrests kept
    // rising (1.10 and 1.10 on the fix round's tree): the arrest count does
    // not track the force at 20k. The broken-system signals are above.
    eprintln!(
        "FINDING god_cut_the_precinct: arrests kept under the cut {:.3} with Stations, {:.3} without",
        on.kept(),
        off.kept()
    );
}
