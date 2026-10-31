//! The city milestone C2: the view's snapshot (`citysim::view`), what the 3D client draws of the camera ring.

mod common;

use citysim::components::{Building, Identity, Rect};
use citysim::systems::motion::{self, vehicle_of};
use citysim::{exec, view, AssetKind, Brain, Lod, World, TICKS_PER_HOUR};

/// Where the snapshot must put a body: at its vehicle's place when aboard, on its way when a Coarse walker.
fn place(w: &World, f: &view::ViewFrame, e: citysim::EntityId) -> Option<citysim::TilePos> {
    match vehicle_of(w, e) {
        Some(v) if w.motions.contains_key(&v) => f.vehicle(v).map(|x| x.t),
        _ => exec::walk_place(w, e).map(|p| p.tile),
    }
}

/// test-add: the view snapshot drops bodies silently. Every Full and Coarse body in the ring is in the frame once,
/// every rider of the ring's vehicles is aboard in it, and every id resolves (a living body, a motion, a building).
#[test]
fn test_snapshot_holds_every_body_in_the_ring() {
    let mut w = common::world();
    w.run_ticks(7 * TICKS_PER_HOUR);
    // riders by hand: the 2k city's commute rides little, so three adults on the street take a cab across the map
    let (mw, mh) = (w.map.w(), w.map.h());
    let far = citysim::TilePos::new((mw - 2) as u16, (mh - 2) as u16);
    let street: Vec<_> = common::adults(&w)
        .into_iter()
        .filter(|&a| w.comp::<citysim::Position>(a).is_some_and(|p| p.building.is_none()))
        .take(3)
        .collect();
    assert!(!street.is_empty(), "nobody on the street at 07:00");
    for a in street {
        let now = w.tick;
        motion::board_timed(&mut w, a, motion::RideKind::Cab, AssetKind::Car, None, None, far, now, now + 120, true)
            .expect("[motion] is on in the shipped city");
    }
    let whole = Rect { x: 0, y: 0, w: mw as u16, h: mh as u16 };
    let half = Rect { x: (mw / 4) as u16, y: (mh / 4) as u16, w: (mw / 2) as u16, h: (mh / 2) as u16 };
    let (mut walkers, mut riders, mut bodies) = (0usize, 0usize, 0usize);
    for i in 0..60 {
        if i > 0 {
            w.run_ticks(1);
        }
        for ring in [whole, half] {
            let f = view::snapshot(&w, ring);
            for e in w.bodies() {
                let lod = w.comp::<Brain>(e).map(|b| b.lod);
                assert!(matches!(lod, Some(Lod::Full | Lod::Coarse)));
                let Some(t) = place(&w, &f, e) else { continue };
                if !ring.contains(t) {
                    continue;
                }
                let n = f.agents.iter().filter(|a| a.id == e).count();
                assert_eq!(n, 1, "tick {}: body {e:?} at {t:?} in the ring {ring:?} is in the frame {n} times", w.tick);
                bodies += 1;
                let a = f.agent(e).unwrap();
                assert_eq!(a.t, t, "tick {}: body {e:?} drawn at {:?}, placed at {t:?}", w.tick, a.t);
                if a.lod == Some(Lod::Coarse) && !a.ahead.is_empty() && a.aboard.is_none() {
                    walkers += 1;
                }
            }
            for v in &f.vehicles {
                assert!(w.motions.contains_key(&v.id), "tick {}: vehicle {:?} is no motion", w.tick, v.id);
                for r in v.riders.iter().chain(v.driver.iter()) {
                    assert!(w.is_alive(*r), "tick {}: vehicle {:?} carries a dead id {r:?}", w.tick, v.id);
                }
                if !ring.contains(v.t) {
                    continue;
                }
                for r in &v.riders {
                    if !w.has::<Identity>(*r) {
                        continue;
                    }
                    let a = f.agent(*r);
                    assert!(
                        a.is_some_and(|a| a.aboard == Some(v.id)),
                        "tick {}: rider {r:?} of {:?} {:?} at {:?} is not aboard in the frame",
                        w.tick,
                        v.ride,
                        v.id,
                        v.t
                    );
                    riders += 1;
                }
            }
            for a in &f.agents {
                assert!(
                    w.is_alive(a.id) && w.has::<Identity>(a.id),
                    "tick {}: agent {:?} does not resolve",
                    w.tick,
                    a.id
                );
                if let Some(b) = a.b {
                    assert!(w.has::<Building>(b), "tick {}: agent {:?} is in {b:?}, no building", w.tick, a.id);
                }
                if let Some(v) = a.aboard {
                    assert!(
                        f.vehicle(v).is_some(),
                        "tick {}: agent {:?} aboard {v:?}, no vehicle in the frame",
                        w.tick,
                        a.id
                    );
                }
            }
        }
    }
    // not vacuous: the commute hour holds bodies, Coarse walkers on their way and riders aboard
    assert!(bodies > 0 && walkers > 0 && riders > 0, "bodies {bodies}, Coarse walkers {walkers}, riders {riders}");
}
