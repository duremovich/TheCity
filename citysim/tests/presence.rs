//! The pre-view pass phase 4 (docs/PREVIEW_PASS.md § 0 P3, P8; OPEN-119,
//! OPEN-120, OPEN-154; `systems/presence.rs`): an encounter posts when, and
//! only when, both bodies are present. Each test is a seeded world and one
//! mechanism (each failed on the base; the switch retired at the island
//! pass phase 0, OPEN-174): the
//! super serves only the tenants there and awake; the pimp collects only
//! from a worker at his pitch; an off-screen buyer's arms sale is made at
//! the meeting with the dealer's body, the witnesses the bodies in the Bar;
//! the sweeper walks a round of street stops and closes his shift at the
//! Recycler; a round's stops lie within its reach.

mod common;

use citysim::components::{AssetKind, AssetLoc, Brain, Lod, MemoryKind, Position, ProvKind, Provenance, ShopPick};
use citysim::systems::{demography, law, lod, service, trades, vice, weapons};
use citysim::{
    BuildingKind, Config, EntityId, EventKind, ExecState, Job, Pitch, Role, World, TICKS_PER_DAY, TICKS_PER_HOUR,
};

fn city() -> World {
    World::new(42, Config::load())
}

fn jobs2(w: &World) -> citysim::stats::Jobs2Cols {
    w.stats.current.jobs2.clone().unwrap_or_default()
}

fn free(w: &World, a: EntityId) -> bool {
    law::living(w, a) && !w.has::<citysim::Sentence>(a) && demography::is_adult(w, a) && w.has::<Brain>(a)
}

/// Stand `who` out on the street at `tile` (no building), a body.
fn stand(w: &mut World, who: EntityId, tile: citysim::TilePos) {
    w.remove_from_building(who);
    let p = w.comp_mut::<Position>(who).expect("position");
    p.tile = tile;
    p.building = None;
}

/// P3, OPEN-120: the super's stop at a Block serves the tenant inside and
/// awake, not the one out of the Block nor the one asleep in it (the base
/// served every adult resident wherever he was).
#[test]
fn test_super_serves_only_a_tenant_present_and_awake() {
    let served = || -> (bool, bool, bool) {
        let mut w = city();
        w.run_ticks(TICKS_PER_DAY + 10 * TICKS_PER_HOUR);
        let role = w.config.trade_role("super").expect("the super trade");
        let s = w
            .citizens()
            .into_iter()
            .find(|&a| free(&w, a) && w.comp::<Job>(a).is_some_and(|j| j.role == role && j.employer.is_some()))
            .expect("a super");
        // A Block with three free adult residents (not the super).
        let (block, t) = w
            .buildings_of_kind(BuildingKind::Home)
            .iter()
            .copied()
            .find_map(|b| {
                let t: Vec<EntityId> = w.residents_of(b).iter().copied().filter(|&r| r != s && free(&w, r)).collect();
                (t.len() >= 3).then_some((b, t))
            })
            .expect("a Block with three adult tenants");
        let (here, away, sleeper) = (t[0], t[1], t[2]);
        for &x in &[here, away, sleeper] {
            lod::set_lod(&mut w, x, Lod::Full);
        }
        w.enter_building(here, block);
        w.enter_building(sleeper, block);
        let now = w.tick;
        w.comp_mut::<Brain>(sleeper).expect("brain").exec =
            ExecState::Use { kind: citysim::ActionKind::Sleep, until: now + 300, started: now };
        let door = w.comp::<citysim::Building>(block).expect("block").door;
        stand(&mut w, away, citysim::TilePos { x: door.x, y: door.y.saturating_add(8) });
        service::super_stop(&mut w, s, block);
        let met = |x: EntityId| service::regulars_of(&w, s).iter().any(|r| r.who == x);
        (met(here), met(away), met(sleeper))
    };
    let (here, away, sleeper) = served();
    assert!(here, "the tenant at home and awake is served");
    assert!(!away, "the tenant out of the Block is not served");
    assert!(!sleeper, "the tenant asleep is not served");
}

/// A street pitch for `owner` at his tile, under pimp `m` (enlisted in the
/// first gang), both bodies; the night's hour of the pitch.
fn pimped_pitch(w: &mut World, take: i64) -> (EntityId, EntityId, Pitch) {
    w.run_ticks(TICKS_PER_DAY + 1);
    let idle: Vec<EntityId> =
        w.citizens().into_iter().filter(|&a| free(w, a) && w.gang_of(a).is_none() && !w.has::<Job>(a)).collect();
    let (o, m) = (idle[2], idle[3]);
    let trade = w.config.trade_role("street_worker").and_then(|r: Role| r.trade()).expect("the street row");
    let at = w.comp::<Position>(o).expect("pos").tile;
    let d = w.district_of(at);
    let p = Pitch {
        spot: at,
        district: d,
        trade,
        owner: o,
        since: w.tick,
        take_today: take,
        take_week: 0,
        cut_to: None,
        stock: 0,
        closed_until: None,
        dry_days: 0,
        jobs_today: 0,
        cut_paid: 0,
    };
    w.jobs2.pitches.insert(o, p.clone());
    let g = w.gangs()[0];
    citysim::systems::gang::enlist(w, m, g);
    w.jobs2.vice.pimps.insert(d.index() as u16, m);
    lod::set_lod(w, m, Lod::Full);
    lod::set_lod(w, o, Lod::Full);
    stand(w, m, p.spot);
    w.tick = TICKS_PER_DAY + 2 * 60;
    (o, m, p)
}

/// P3, OPEN-120, OPEN-119: the pimp's collection needs both bodies: with
/// the worker away from his pitch nothing is collected and no encounter
/// posts; with him at it, the collection posts and the cut moves then
/// (`cut_at_meet`), worker to pimp. The base collected from the absent
/// worker.
#[test]
fn test_pimp_collects_only_from_a_worker_at_his_pitch() {
    let collect = |at_pitch: bool| -> (u32, u32, i64) {
        let mut w = city();
        let (o, m, p) = pimped_pitch(&mut w, 100);
        if at_pitch {
            stand(&mut w, o, p.spot);
        } else {
            let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
            w.enter_building(o, bar);
        }
        let before = jobs2(&w);
        let purse = common::coins(&w, o);
        vice::pimp_collect(&mut w, m, Some(o));
        let after = jobs2(&w);
        (
            after.pimp_collections - before.pimp_collections,
            after.cut_at_meet - before.cut_at_meet,
            purse - common::coins(&w, o),
        )
    };
    let (n, cut, _) = collect(false);
    assert_eq!((n, cut), (0, 0), "an absent worker: no collection, no cut");
    let (n, cut, paid) = collect(true);
    assert_eq!((n, cut), (1, 1), "the worker at his pitch: collected, the cut taken at the meeting");
    assert!(paid > 0, "the cut moved from the worker's purse at the meeting");
}

/// P3, OPEN-119: an off-screen buyer's weapon from a dealer who is a body
/// is booked, not sold, at midnight; it is sold at the first hour the
/// dealer's body is in the Bar, and the crime is seen by the bodies there
/// (the witnesses chosen at the meeting). The base sold it at the booking,
/// the dealer asleep at home, the Bar's bodies none the wiser.
#[test]
fn test_arms_sale_posts_at_the_meeting_with_the_bodies_present() {
    let sale = || -> (Option<u64>, bool, u64) {
        let mut w = city();
        w.run_ticks(TICKS_PER_DAY + 1);
        w.config.weapons.armoury_per_member = 0.0;
        w.config.crime.witness_base = 1.0;
        w.config.crime.witness_stealth_factor = 0.0;
        let g = w.gangs().into_iter().find(|&g| w.hideout_of(g).is_some()).expect("a gang with a Hideout");
        let h = w.hideout_of(g).expect("hideout");
        let prov = Provenance { how: ProvKind::Granted, by: None, at: w.tick };
        weapons::spawn_weapon(&mut w, 1, Some(g), AssetLoc::Stock(h), prov, true).expect("a weapon");
        let civ: Vec<EntityId> =
            w.citizens().into_iter().filter(|&a| free(&w, a) && w.gang_of(a).is_none() && !w.has::<Job>(a)).collect();
        let (d, buyer, witness) = (civ[0], civ[1], civ[2]);
        citysim::systems::gang::enlist(&mut w, d, g);
        let bar = w.buildings_of_kind(BuildingKind::Bar)[0];
        let day = w.day();
        w.deal_log.insert(bar, (d, day));
        lod::set_lod(&mut w, d, Lod::Full);
        lod::set_lod(&mut w, witness, Lod::Full);
        lod::set_lod(&mut w, buyer, Lod::Statistical);
        w.comp_mut::<citysim::Wallet>(buyer).expect("wallet").coins = 100_000;
        let home = w.comp::<citysim::Household>(d).and_then(|h| h.home).expect("a housed dealer");
        w.enter_building(d, home);
        w.enter_building(witness, bar);
        let pick = ShopPick { kind: AssetKind::Weapon, tier: 1, used: None, upgrade: false };
        let booked_at = w.tick;
        if !weapons::book_meeting(&mut w, buyer, bar, &pick) {
            let _ = weapons::buy(&mut w, buyer, bar, &pick, false);
        }
        // Two hours with the dealer at home, then he walks into the Bar.
        for hour in 1..=4u64 {
            w.tick = (booked_at / TICKS_PER_HOUR + hour) * TICKS_PER_HOUR;
            if hour == 3 {
                w.enter_building(d, bar);
                w.dealers.entry(bar).or_default().push(d);
            }
            weapons::arms_meetings(&mut w);
        }
        let sold = w
            .events
            .iter()
            .find(|e| e.kind == EventKind::AssetBought && e.actors.first() == Some(&buyer))
            .map(|e| e.tick);
        let seen = w
            .comp::<citysim::Memory>(witness)
            .is_some_and(|m| m.entries.iter().any(|e| e.kind == MemoryKind::SawCrime && e.subject == Some(d)));
        (sold, seen, (booked_at / TICKS_PER_HOUR + 3) * TICKS_PER_HOUR)
    };
    let (sold, seen, meeting) = sale();
    assert_eq!(sold, Some(meeting), "sold at the meeting hour, with the dealer's body in the Bar");
    assert!(seen, "the body in the Bar witnessed the sale");
}

/// P8: a body sweeper's shift is a round of at least two street stops of
/// his beat, swept in turn, ending at the Recycler (the post), where the
/// shift closes. The base swept one stand to the shift's end.
#[test]
fn test_sweeper_walks_street_stops_and_ends_at_the_recycler() {
    let walk = || -> (usize, bool, bool) {
        let mut w = city();
        w.run_ticks(TICKS_PER_DAY + 2 * TICKS_PER_HOUR);
        let s = w
            .citizens()
            .into_iter()
            .find(|&a| {
                free(&w, a)
                    && w.comp::<Job>(a).is_some_and(|j| {
                        j.role == Role::Sanitation
                            && j.employer.is_some()
                            && j.shifts.iter().any(|&(st, _)| u64::from(st) >= 6 * 60)
                            && citysim::exec::routine::workday_of(&w, a, j, j.next_shift_key(w.tick))
                    })
            })
            .expect("a day sweeper on a workday");
        lod::set_lod(&mut w, s, Lod::Full);
        let job = w.comp::<Job>(s).expect("job").clone();
        let post = job.employer.expect("the Recycler");
        let (start, end) = job.shifts[0];
        let day0 = w.tick / TICKS_PER_DAY * TICKS_PER_DAY;
        let close = day0 + u64::from(end);
        // The precondition pinned: he starts his shift at the Recycler's door
        // (a sweeper starting past a shift's walk from it works the old stand,
        // `sweep_stands`).
        w.run_ticks((day0 + u64::from(start)).saturating_sub(w.tick + 1));
        let door = w.comp::<citysim::Building>(post).expect("the Recycler").door;
        stand(&mut w, s, door);
        let mut stops = std::collections::BTreeSet::new();
        let mut at_post_end = false;
        while w.tick < close + 30 {
            w.comp_mut::<Brain>(s).expect("brain").pinned = true;
            w.run_ticks(1);
            let tod = w.tick_of_day();
            let sweeping = w
                .comp::<Brain>(s)
                .is_some_and(|b| matches!(b.exec, ExecState::Use { kind: citysim::ActionKind::Sweep, .. }));
            let pos = w.comp::<Position>(s).cloned().expect("pos");
            if sweeping && tod >= start && tod < end && pos.building.is_none() {
                stops.insert(pos.tile);
            }
            if tod + 1 == end {
                at_post_end = pos.building == Some(post);
            }
        }
        let closed =
            w.comp::<Job>(s).is_some_and(|j| j.last_shift_day == Some(job.shift_key_at(day0 + u64::from(start))));
        (stops.len(), at_post_end, closed)
    };
    let (stops, at_post, closed) = walk();
    eprintln!("sweeper: {stops} street stops, at the Recycler at the end {at_post}, closed {closed}");
    assert!(stops >= 2, "the round swept {stops} street stops");
    assert!(at_post, "the shift ends at the Recycler");
    assert!(closed, "the shift closed (the wage) at the post");
}

/// OPEN-154: every round's stops lie within `[presence] round_reach_tiles`
/// of its post's door (the super's, the vendor's, the tinker's: `round_for`,
/// the list a body walks and the Statistical super is credited).
#[test]
fn test_round_stays_within_its_reach() {
    let far = || -> (u32, usize) {
        let mut w = city();
        w.run_ticks(TICKS_PER_DAY + 8 * TICKS_PER_HOUR);
        let mut worst = 0;
        let mut rounds = 0;
        for a in w.citizens() {
            if !trades::on_round(&w, a) {
                continue;
            }
            let Some(j) = w.comp::<Job>(a) else { continue };
            let Some(post) = j.employer else { continue };
            let door = w.comp::<citysim::Building>(post).expect("post").door;
            let key = j.next_shift_key(w.tick);
            let yard = matches!(
                w.comp::<citysim::Building>(post).map(|b| b.kind),
                Some(BuildingKind::ConstructionYard | BuildingKind::FreightYard)
            );
            if yard {
                continue;
            }
            let stops = trades::round_for(&w, a, key);
            rounds += usize::from(!stops.is_empty());
            for b in stops {
                worst = worst.max(w.comp::<citysim::Building>(b).expect("stop").door.manhattan(door));
            }
        }
        (worst, rounds)
    };
    let reach = Config::load().presence.expect("[presence]").round_reach_tiles;
    let (worst, rounds) = far();
    eprintln!("rounds {rounds}: the farthest stop {worst} tiles from its post (reach {reach})");
    assert!(rounds > 0, "some round has stops");
    assert!(worst <= reach, "a stop {worst} tiles from the post");
}

/// The phase's device rows, printed (tier: diag): seed 42 x 15 days with
/// `[presence]`: each body super's walking minutes a
/// shift (p50/p90/max), and the sweepers' "nearer work" re-housings (lines
/// and nights; phase 1 read 6-8 of 15 nights).
#[test]
#[ignore = "tier: diag"]
fn diag_presence_device_rows() {
    fn rows() -> String {
        let mut w = city();
        let role = w.config.trade_role("super").expect("the super trade");
        let mut walk: std::collections::BTreeMap<(EntityId, i64), u32> = Default::default();
        let mut supers: Vec<EntityId> = Vec::new();
        let mut moved = 0u32;
        let mut nights = std::collections::BTreeSet::new();
        let mut movers = std::collections::BTreeSet::new();
        let mut seen = 0u64;
        let (mut sweep_shifts, mut sweep_closed) = (0u32, 0u32);
        while w.tick < 15 * TICKS_PER_DAY {
            if w.tick.is_multiple_of(TICKS_PER_DAY) {
                supers = w
                    .citizens()
                    .into_iter()
                    .filter(|&a| w.comp::<Job>(a).is_some_and(|j| j.role == role && j.employer.is_some()))
                    .collect();
            }
            w.run_ticks(1);
            let tod = w.tick_of_day();
            for &s in &supers {
                let Some(j) = w.comp::<Job>(s) else { continue };
                let body = w.comp::<Brain>(s).is_some_and(|b| b.lod != Lod::Statistical);
                if !body || !j.on_shift(tod) {
                    continue;
                }
                let key = j.shift_key_at(w.tick);
                let e = walk.entry((s, key)).or_default();
                if w.comp::<Brain>(s).is_some_and(|b| {
                    matches!(b.exec, ExecState::Goto { .. } | ExecState::GotoTimed { .. } | ExecState::Fly { .. })
                }) {
                    *e += 1;
                }
            }
            for ev in w.events.iter().rev().take_while(|e| e.id >= seen) {
                if ev.kind == EventKind::Housed && ev.text.contains("nearer work") {
                    let s = ev.actors.first().copied().unwrap_or(EntityId::NONE);
                    if w.comp::<Job>(s).is_some_and(|j| j.role == Role::Sanitation) {
                        moved += 1;
                        movers.insert(s);
                        nights.insert(ev.tick / TICKS_PER_DAY);
                    }
                }
            }
            seen = w.events.back().map_or(seen, |e| e.id + 1);
            // Body sweepers' shifts: worked (closed) or not, read at each end.
            if w.tick % 60 == 5 {
                for a in w.citizens() {
                    let Some(j) = w.comp::<Job>(a) else { continue };
                    if j.role != Role::Sanitation || w.comp::<Brain>(a).is_none_or(|b| b.lod == Lod::Statistical) {
                        continue;
                    }
                    let tod = w.tick_of_day();
                    if j.shifts.iter().any(|&(_, e)| e % 1440 + 5 == tod) {
                        let key = j.shift_key_at(w.tick - 6);
                        if citysim::exec::routine::workday_of(&w, a, j, key) {
                            sweep_shifts += 1;
                            sweep_closed += u32::from(j.last_shift_day == Some(key));
                        }
                    }
                }
            }
        }
        let mut v: Vec<u32> = walk.values().copied().collect();
        v.sort_unstable();
        let q = |p: f64| v.get(((v.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0);
        format!(
            "super walking min/shift n {} p50 {} p90 {} max {}; sweeper nearer-work lines {moved} ({} sweepers) on {} nights; body sweeper shifts closed {sweep_closed} of {sweep_shifts}",
            v.len(),
            q(0.5),
            q(0.9),
            v.last().copied().unwrap_or(0),
            movers.len(),
            nights.len()
        )
    }
    eprintln!("FINDING presence: {}", rows());
}
