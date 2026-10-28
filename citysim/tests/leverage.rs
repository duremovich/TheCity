//! M16b § 4 (docs/M16_CONTRACTS.md; phase 1): leverage. One test per
//! mechanism, in the shipped seed-42 city: the shakedown's roll unchanged,
//! the move's terms (a ransom above the purse refused, a secret's weight),
//! order independence, protection paid weekly and lost with the district
//! (never on the gang's own front), blackmail and its lapse, the secret
//! tally, Silence, Debt, poaching by threat, Parley both ways, a coerced
//! job, fraud (the renege's filing, the fake job), the Court suitor's
//! `Persuade`, the coin identity and a save mid-coercion. Every coercion
//! here is game state between fictional agents: a record on a struct and a
//! seeded dice roll. With `[leverage] enabled = false` every mechanism test
//! fails (the failing-first device: the switch gates every entry point).

use citysim::contract::{
    CoercionStatus, CoercionWhy, ContractKind, ContractStatus, Demand, Leverage, Motive, Origin, PostExtra, Posting,
    Target, Terms,
};
use citysim::systems::econ::identity;
use citysim::systems::{contracts, demography, gang, law, leverage, moves};
use citysim::word::{Deed, MoveKind, PoolEntry, SocialMove, Stake};
use citysim::{
    Building, BuildingKind, Config, Crime, EntityId, EventKind, Gang, GangMember, MemoryEntry, MemoryKind, Personality,
    RelKind, Skills, World, TICKS_PER_DAY,
};

/// The shipped city with the outside's parents off (M17 phase 2): these
/// seeded-world tests read the M16b city's gangs and businesses at day ~40
/// (which gang is the rival, which business is the district's best), and the
/// parents move that trajectory.
fn world() -> World {
    world_trains(true)
}

/// `world`, the El's trains on or off (`[motion.trains]`).
fn world_trains(trains: bool) -> World {
    let mut c = Config::load();
    if !trains {
        if let Some(m) = c.motion.as_mut() {
            m.trains = None;
        }
    }
    c.outside.enabled = false;
    // Jobs & lives phase 1: and without the industries (they take the Lots
    // and the jobs that set this city's gangs and businesses).
    c.jobs2.enabled = false;
    // The pre-view pass phase 6: the protection paid off screen (midnight)
    // and the orders in force at once, the moves this file tests (the
    // leader's round and court are `leaders.rs`'s).
    c.leaders = None;
    World::new(42, c)
}

/// The seed-42 city once its gangs hold Homes and have leaders (two or
/// more led gangs, one holding a district with a business), run once and
/// shared as a clone of the live world (OPEN-112: a save of it resumes
/// byte for byte, so `test_save_mid_coercion_round_trips` saves a live
/// world, not one already loaded).
fn gang_world() -> World {
    static CITY: std::sync::OnceLock<World> = std::sync::OnceLock::new();
    gang_world_in(&CITY, false)
}

/// [`gang_world`] with the pre-mirror off-screen evening throughout (a
/// stat table without `p_venue`, the best-rung path).
fn gang_world_pre_mirror() -> World {
    static CITY: std::sync::OnceLock<World> = std::sync::OnceLock::new();
    gang_world_in(&CITY, true)
}

fn no_venue_rates(w: &mut World) {
    if let Some(t) = w.stat_table.as_mut() {
        t.p_venue = None;
    }
}

fn gang_world_in(cell: &std::sync::OnceLock<World>, pre_mirror: bool) -> World {
    let base = cell.get_or_init(|| {
        // The island pass phase 6: the pre-mirror city is pinned on the El's
        // timed ride while the trains' switch is in flight (with trains its
        // racket's hold broke in the second week); re-read when it retires.
        let mut w = world_trains(!pre_mirror);
        // These tests pin M16b's protection (the turf's districts under
        // Expand or Contest, a refusal no hold, the leaner's fall the hold's
        // end): the pressure pass phase 3 ([protection]) moved the 2k city and its semantics.
        w.config.protection = None;
        if pre_mirror {
            no_venue_rates(&mut w);
        }
        // Jobs & lives phase 7b (the regenerated stat table): the premise is
        // a city of established gangs: two led gangs of two or more, one
        // holding three Homes or more in a district (under the new table the
        // first day with two led gangs and one Home held came on day 6, a
        // one-member rival and a single Home that flipped within two days).
        for _ in 0..40 {
            w.run_ticks(TICKS_PER_DAY);
            let led = w
                .gangs()
                .into_iter()
                .filter(|&g| w.comp::<Gang>(g).is_some_and(|x| x.leader.is_some() && x.members.len() >= 2))
                .count();
            let held = w.gangs().into_iter().any(|g| gang::held_districts(&w, g).iter().any(|&(_, n)| n >= 3));
            // Shadow V4 batch 3: and a led gang holding a district with a
            // business to protect (`racket`'s premise; the moved city's
            // first such day came later than its first held district).
            let business = w.gangs().into_iter().any(|g| {
                w.comp::<Gang>(g).is_some_and(|x| x.leader.is_some())
                    && gang::held_districts(&w, g).into_iter().any(|(d, _)| {
                        leverage::PROTECTED.iter().flat_map(|&k| w.buildings_of_kind(k).to_vec()).any(|b| {
                            w.district_of_building(b) == d
                                && w.comp::<Building>(b)
                                    .is_some_and(|bd| !bd.derelict && !bd.demolished && bd.venue.is_none())
                        })
                    })
            });
            if led >= 2 && held && business {
                break;
            }
        }
        w
    });
    let mut w = base.clone();
    if pre_mirror {
        no_venue_rates(&mut w);
    }
    w
}

fn adults(w: &World) -> Vec<EntityId> {
    w.citizens().into_iter().filter(|&id| law::living(w, id) && demography::is_adult(w, id)).collect()
}

/// `n` free adults with no edge between any two, no gang, no guard's job.
fn strangers(w: &World, n: usize) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = Vec::new();
    for a in adults(w) {
        if w.has::<GangMember>(a) || law::is_guard(w, a) || w.has::<citysim::Sentence>(a) {
            continue;
        }
        if w.spouse_of(a).is_some() || out.iter().any(|&o| w.edge(o, a).is_some()) {
            continue;
        }
        out.push(a);
        if out.len() == n {
            return out;
        }
    }
    panic!("{n} strangers");
}

fn set_coins(w: &mut World, id: EntityId, coins: i64) {
    if let Some(x) = w.comp_mut::<citysim::Wallet>(id) {
        x.coins = coins;
    }
}

/// A hard, lawless, fearless lean-on-people type.
fn heavy(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.lawfulness = 0.0;
        p.courage = 1.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.fighting = 1.0;
        s.intimidation = 1.0;
        s.persuasion = 1.0;
        s.deception = 1.0;
    }
}

/// A timid, soft mark.
fn soft(w: &mut World, id: EntityId) {
    if let Some(p) = w.comp_mut::<Personality>(id) {
        p.courage = 0.0;
        p.greed = 1.0;
    }
    if let Some(s) = w.comp_mut::<Skills>(id) {
        s.fighting = 0.0;
        s.knowledge = 0.0;
    }
}

/// Retry `f` on later ticks (each move is one keyed draw per tick) until it
/// succeeds; the tried-pair locks cleared between tries.
fn until_ok(w: &mut World, mut f: impl FnMut(&mut World) -> bool) -> bool {
    for _ in 0..200 {
        w.lev_tried.clear();
        if f(w) {
            return true;
        }
        w.tick += 1;
    }
    false
}

/// Run to the next midnight's leverage pass (one tick past it).
fn to_midnight(w: &mut World) {
    let left = TICKS_PER_DAY - w.tick % TICKS_PER_DAY;
    w.run_ticks(left + 1);
}

fn clone(w: &World) -> World {
    citysim::save::from_ron(&citysim::save::to_ron(w)).expect("round trip")
}

fn has_event(w: &World, kind: EventKind, who: EntityId) -> bool {
    w.events.iter().any(|e| e.kind == kind && e.actors.contains(&who))
}

/// The shakedown (M8, M15 W27) is the same roll: the leverage terms of a
/// one-off coin demand up to twice `extort_amount` are 0, and the outcome
/// and `p` match M15's call on the same world.
#[test]
fn test_shakedown_is_the_same_roll() {
    let mut w = world();
    let [a, t] = strangers(&w, 2)[..] else { unreachable!() };
    let amount = w.config.social.extort_amount;
    let x = leverage::extra(&w, t, None, &Leverage::Threat, &Demand::Coins { amount, weekly: false });
    assert_eq!(x, 0.0, "a shakedown's demand costs nothing");
    let mut w2 = clone(&w);
    let m = SocialMove { actor: a, target: t, kind: MoveKind::Intimidate, stake: Stake::Coins(amount) };
    for _ in 0..20 {
        let ours = leverage::shakedown(&mut w, a, t, amount);
        let m15 = moves::resolve(&mut w2, &m);
        assert_eq!((ours.success, ours.p), (m15.success, m15.p));
        w.tick += 1;
        w2.tick += 1;
    }
}

/// Spec § 4: a ransom above the purse is refused (p at the floor); a weekly
/// demand costs more than a one-off of the same coins; a secret adds its
/// weight.
#[test]
fn test_move_terms_ransom_and_secret() {
    let mut w = world();
    let [a, t, o] = strangers(&w, 3)[..] else { unreachable!() };
    set_coins(&mut w, t, 0);
    let x = leverage::extra(&w, t, None, &Leverage::Threat, &Demand::Coins { amount: 100_000, weekly: false });
    assert!(x < -10.0, "the ransom's cost {x}");
    let out = leverage::roll(&mut w, a, t, None, &Leverage::Threat, &Demand::Coins { amount: 100_000, weekly: false });
    assert!((out.p - w.config.moves.p_min).abs() < 1e-6, "p {} at the floor", out.p);
    set_coins(&mut w, t, 500);
    let one = leverage::extra(&w, t, None, &Leverage::Threat, &Demand::Coins { amount: 100, weekly: false });
    let weekly = leverage::extra(&w, t, None, &Leverage::Threat, &Demand::Coins { amount: 100, weekly: true });
    assert!(weekly < one, "weekly {weekly} costs more than one-off {one}");
    let secret = Leverage::Secret { deed: Deed::Killed, actor: t, object: Some(o), day: 0 };
    let s = leverage::extra(&w, t, None, &secret, &Demand::Coins { amount: 100, weekly: true });
    let sev = w.config.gossip.deed_sev.get(Deed::Killed);
    let want = weekly + w.config.leverage.lev_bias.secret + w.config.leverage.secret_w * sev;
    assert!((s - want).abs() < 1e-5, "secret {s} vs {want}");
}

/// `coerce` is order-independent: a move's outcome does not depend on the
/// moves made before it in the tick.
#[test]
fn test_coerce_is_order_independent() {
    let w = world();
    let [a, b, c, d] = strangers(&w, 4)[..] else { unreachable!() };
    let demand = Demand::Coins { amount: 30, weekly: true };
    let mut w1 = clone(&w);
    let first = leverage::roll(&mut w1, a, b, None, &Leverage::Threat, &demand);
    let mut w2 = clone(&w);
    let _ = leverage::roll(&mut w2, c, d, None, &Leverage::Threat, &demand);
    let second = leverage::roll(&mut w2, a, b, None, &Leverage::Threat, &demand);
    assert_eq!((first.success, first.p), (second.success, second.p));
}

/// A gang, a held district and a protected business there owned by an
/// agent, with a week's revenue.
fn racket(w: &mut World) -> (EntityId, EntityId, EntityId, EntityId) {
    for g in w.gangs() {
        let Some(leader) = w.comp::<Gang>(g).and_then(|x| x.leader) else { continue };
        for (d, _) in gang::held_districts(w, g) {
            let found = leverage::PROTECTED.iter().flat_map(|&k| w.buildings_of_kind(k).to_vec()).find(|&b| {
                w.district_of_building(b) == d
                    && w.comp::<Building>(b).is_some_and(|bd| !bd.derelict && !bd.demolished && bd.venue.is_none())
            });
            let Some(b) = found else { continue };
            let owner = strangers(w, 1)[0];
            // Jobs & lives phase 7b (the regenerated stat table): the deed
            // through `transfer_building`; a bare `owner` write skipped the
            // books, and by the next week the building had no owner (payer
            // None) and the hold read Broken before the district was lost.
            citysim::systems::ownership::transfer_building(w, b, Some(owner));
            if let Some(bd) = w.comp_mut::<Building>(b) {
                bd.secured_by = None;
                bd.revenue = std::iter::repeat_n(100, 7).collect();
            }
            heavy(w, leader);
            return (g, leader, b, owner);
        }
    }
    panic!("no gang holds a district with a business");
}

/// Protection is paid weekly from the owner's purse to the gang, three
/// weeks running, inside the coin identity; it stops (`Broken`) when the
/// gang no longer holds the district.
#[test]
fn test_protection_pays_weekly_and_stops_when_district_lost() {
    // Jobs & lives phase 7b: the pre-mirror off-screen evening (no `p_venue`): the learned Bar visits fed the leader's black-BD dealing and its arrest, not this test's protection.
    let mut w = gang_world_pre_mirror();
    let (g, leader, b, owner) = racket(&mut w);
    // Round 4 T4 (premise moved): with 10,000 the owner was the richest
    // purse at a corp's bankruptcy (`corps` estate sale, d15), bought its
    // Noodle Bar, incorporated, and the Hotel changed hands: the hold broke.
    // This test is the weekly payment: a purse that pays the weeks and
    // loses an estate sale to the corps' spare treasuries.
    set_coins(&mut w, owner, 600);
    // Shadow V4 batch 1: on the moved city the owner spent the 10,000 on a
    // founding and incorporated, so the Hotel changed hands (payer) and the
    // hold broke; she has just founded (the cooldown), and this is the
    // payment's test.
    let today = w.day();
    w.comp_mut::<citysim::Brain>(owner).expect("brain").last_found_day = Some(today);
    let before = identity(&w);
    let weekly = w.config.leverage.protection_weekly;
    let day0 = w.day();
    // The pressure pass round 2 (premise moved): on the combined city the
    // two led gangs flip each other's Blocks week on week (R4's friendship
    // flip: only the holders who see a blow turn Enemy), and the racket's
    // district changed hands in the second week, ending the hold before the
    // test's loss. This test is the weekly payment, so the turf is held as
    // staged; the loss is the explicit removal below.
    let held_turf: Vec<EntityId> = w.comp::<Gang>(g).unwrap().territory.clone();
    let id = leverage::force(
        &mut w,
        Some(g),
        leader,
        owner,
        Leverage::Threat,
        Demand::Coins { amount: weekly, weekly: true },
        CoercionWhy::Protection,
        Some(owner),
        Some(b),
    );
    assert!(leverage::on(&w), "the switch is on");
    assert_eq!(w.coercions[&id].paid, 1, "the first week at once");
    for _ in 0..2 {
        w.coercions.get_mut(&id).unwrap().next_due = w.tick;
        // Shadow V4 batch 1: the heavied leader is the block's bruiser, and
        // on the moved city a robot jailed him for a shakedown the next day
        // (a Sentence breaks the hold). This test is the weekly payment, not
        // his record: he walks free.
        // The pressure pass R4 (premise moved): with the 10,000 the owner
        // bought a shut Noodle Bar (phase 6's market), incorporated, and the
        // Bar changed hands: no shut business is for sale in this test.
        let left = TICKS_PER_DAY - w.tick % TICKS_PER_DAY;
        for _ in 0..=left {
            w.underwater.shut.clear();
            w.run_ticks(1);
            w.comp_mut::<Gang>(g).unwrap().territory.clone_from(&held_turf);
            if w.has::<citysim::Sentence>(leader) {
                citysim::systems::law::release(&mut w, leader, false);
            }
        }
    }
    let c = &w.coercions[&id];
    assert_eq!(c.status, CoercionStatus::Active);
    assert!(c.paid >= 3, "paid {} weeks", c.paid);
    // Shadow V4 batch 2: the payments booked as protection (the gang's purse
    // against its opening read the gang's own spending too, which on the
    // moved city outran three weeks of protection).
    let booked: i64 =
        w.stats.history.iter().filter(|r| r.day >= day0).map(|r| r.leverage.protection_coins).sum::<i64>()
            + w.stats.current.leverage.protection_coins;
    assert!(booked >= 3 * weekly, "the gang took it ({booked})");
    assert_eq!(identity(&w), before, "no coin made or lost");
    // The gang loses the district.
    let d = w.district_of_building(b);
    let homes: Vec<EntityId> = w.comp::<Gang>(g).unwrap().territory.clone();
    let (lost, keep): (Vec<EntityId>, Vec<EntityId>) = homes.into_iter().partition(|&h| w.district_of_building(h) == d);
    w.comp_mut::<Gang>(g).unwrap().territory = keep;
    for h in lost {
        if let Some(bd) = w.comp_mut::<Building>(h) {
            bd.claim = None;
        }
    }
    w.coercions.get_mut(&id).unwrap().next_due = w.tick;
    // Straight to the midnight tick (no day for the gang to win a Home back).
    w.tick = (w.tick / TICKS_PER_DAY + 1) * TICKS_PER_DAY;
    w.run_ticks(citysim::systems::midnight::window(&w));
    assert_eq!(w.coercions[&id].status, CoercionStatus::Broken, "lost with the district");
}

/// Protection targets the best business in the held district, never a front
/// of the same gang; a rival gang's front is a target.
#[test]
fn test_protection_never_on_own_front() {
    let mut w = gang_world();
    let (g, _, b, _) = racket(&mut w);
    let d = w.district_of_building(b);
    // Shadow V4: the racket's business the district's best (on the moved
    // city another business there out-earned its staged 100 a day).
    if let Some(bd) = w.comp_mut::<Building>(b) {
        bd.revenue = std::iter::repeat_n(100_000, 7).collect();
        // Pressure pass phase 6: a business under water closes, and a closed
        // one is no target; the staged racket stands open.
        bd.closed_until = None;
    }
    assert_eq!(leverage::protection_target(&w, g, d), Some(b));
    if let Some(bd) = w.comp_mut::<Building>(b) {
        bd.owner = Some(g);
        bd.venue = Some(citysim::living::Venue { front_of: Some(g), ..Default::default() });
    }
    assert_ne!(leverage::protection_target(&w, g, d), Some(b), "never its own front");
    let rival = w.gangs().into_iter().find(|&x| x != g).expect("a rival gang");
    if let Some(bd) = w.comp_mut::<Building>(b) {
        // L2: a front stands on the city's deed, `front_of` the gang.
        bd.owner = None;
        bd.venue = Some(citysim::living::Venue { front_of: Some(rival), ..Default::default() });
    }
    assert_eq!(leverage::protection_target(&w, g, d), Some(b), "a rival's front is a target");
    assert_eq!(leverage::payer_of(&w, b), Some(rival), "the rival gang pays");
}

/// The leader leans on the owner (a roll): when it lands the first week is
/// paid and the deed `Extorted` goes round.
#[test]
fn test_protection_move_made() {
    let mut w = gang_world();
    let (g, leader, b, owner) = racket(&mut w);
    set_coins(&mut w, owner, 10_000);
    soft(&mut w, owner);
    assert!(until_ok(&mut w, |w| leverage::try_protection(w, g, leader, b)), "the move landed");
    let c = w.coercions.values().find(|c| c.building == Some(b)).expect("recorded");
    assert_eq!((c.why, c.paid, c.holder), (CoercionWhy::Protection, 1, Some(g)));
    assert!(has_event(&w, EventKind::Coerced, owner));
}

fn rumour(w: &mut World, holder: EntityId, deed: Deed, actor: EntityId, object: EntityId, hops: u8) {
    let e = MemoryEntry {
        subject: Some(actor),
        salience: 0.9,
        valence: -0.5,
        deed: Some(deed),
        object: Some(object),
        hops,
        conf: 1.0,
        ..MemoryEntry::blank(MemoryKind::Rumour, w.tick)
    };
    citysim::systems::memory::hear_entry(w, holder, e);
}

/// The secret tally: a deed held first-hand or at one hop is a secret; it
/// stops being one when more than `secret_known_max` hold it.
#[test]
fn test_secret_known_to_few_only() {
    let mut w = world();
    let ids = strangers(&w, 10);
    let (a, t, o) = (ids[0], ids[1], ids[2]);
    rumour(&mut w, a, Deed::Killed, t, o, 1);
    let found = leverage::secrets_of(&w, a);
    assert!(found.iter().any(|r| r.deed == Deed::Killed && r.actor == Some(t)), "{found:?}");
    assert!(matches!(leverage::secret_on(&w, a, t), Some(Leverage::Secret { deed: Deed::Killed, .. })));
    for &h in &ids[3..10] {
        rumour(&mut w, h, Deed::Killed, t, o, 2);
    }
    assert!(leverage::secret_on(&w, a, t).is_none(), "known to eight: no secret");
}

/// Blackmail: hush money weekly from the coerced's wallet to the holder; it
/// lapses (no backlash) when the deed's pool reach passes the bound.
#[test]
fn test_blackmail_pays_then_lapses_when_public() {
    let mut w = world();
    let [a, t, o] = strangers(&w, 3)[..] else { unreachable!() };
    heavy(&mut w, a);
    soft(&mut w, t);
    set_coins(&mut w, t, 1000);
    rumour(&mut w, a, Deed::Killed, t, o, 0);
    let lev = leverage::secret_on(&w, a, t).expect("a secret");
    let before = w.purse(Some(a));
    assert!(until_ok(&mut w, |w| leverage::try_blackmail(w, a, t, lev)), "the blackmail landed");
    let id = *w.coercions.iter().find(|(_, c)| c.why == CoercionWhy::Blackmail).unwrap().0;
    assert_eq!(w.purse(Some(a)), before + w.config.leverage.blackmail_weekly, "the first week");
    // The secret goes public.
    let d = citysim::systems::gossip::talk_district(&w, t);
    w.rumours[d.index()].entries.push(PoolEntry {
        deed: Deed::Killed,
        actor: Some(t),
        object: Some(o),
        tick: w.tick,
        hops: 1,
        reach: 0.9,
        hole: None,
        story: None,
        kin: Default::default(),
        told: Default::default(),
        district: d,
        press: 0,
    });
    to_midnight(&mut w);
    assert_eq!(w.coercions[&id].status, CoercionStatus::Lapsed);
    assert!(has_event(&w, EventKind::Lapsed, t));
}

/// Silence: a suspect leans on the witness and the report is withdrawn
/// (the warrant goes with it).
#[test]
fn test_silence_withdraws_the_report() {
    let mut w = world();
    let [s, wit] = strangers(&w, 2)[..] else { unreachable!() };
    heavy(&mut w, s);
    soft(&mut w, wit);
    law::file_report(&mut w, Crime::Assault, s, Some(wit));
    assert!(law::wanted(&w, s));
    assert!(until_ok(&mut w, |w| leverage::try_silence(w, s, wit)), "the threat landed");
    assert!(!law::wanted(&w, s), "the report was withdrawn");
    assert_eq!(w.stats.current.leverage.silenced_threat, 1);
}

/// Debt: a creditor calls an aged debt; the debtor pays what is owed and
/// the edge settles; coins conserved.
#[test]
fn test_debt_called_settles_the_edge() {
    let mut w = world();
    let [c, d] = strangers(&w, 2)[..] else { unreachable!() };
    heavy(&mut w, c);
    soft(&mut w, d);
    set_coins(&mut w, d, 100);
    {
        let (lo, _) = citysim::components::edge_key(c, d);
        let e = w.edge_entry(c, d);
        e.kind = RelKind::Acquaintance;
        e.debt = if lo == d { 10 } else { -10 };
    }
    let before = identity(&w);
    let start = w.purse(Some(c));
    assert!(
        until_ok(&mut w, |w| {
            leverage::call_debts(w, &[(c, d, 10)]);
            w.edge(c, d).is_some_and(|e| e.debt == 0)
        }),
        "the debt was called in"
    );
    assert_eq!(w.purse(Some(c)), start + 10);
    assert_eq!(identity(&w), before);
}

/// Poaching by threat: an exec under `poach_threat_lawfulness` leans
/// (`Some`), a lawful one offers the premium (`None`).
#[test]
fn test_poach_by_threat_only_for_dirty_exec() {
    let mut w = world();
    let [e, t] = strangers(&w, 2)[..] else { unreachable!() };
    let b = w.buildings_of_kind(BuildingKind::Market)[0];
    if let Some(p) = w.comp_mut::<Personality>(e) {
        p.lawfulness = 0.9;
    }
    assert!(leverage::poach(&mut w, e, t, b, 6).is_none(), "a lawful exec pays the premium");
    if let Some(p) = w.comp_mut::<Personality>(e) {
        p.lawfulness = 0.1;
    }
    assert!(leverage::poach(&mut w, e, t, b, 6).is_some(), "a dirty exec leans");
    assert_eq!(w.stats.current.leverage.poach_threats, 1);
}

fn two_gangs(w: &mut World) -> (EntityId, EntityId) {
    // The pre-view pass phase 5: led by a free leader (an escort now takes
    // its walk or the van's drive, not 60 min at most: the shared city's
    // first leader was in cuffs on the way to the cells).
    let gs: Vec<EntityId> = w
        .gangs()
        .into_iter()
        .filter(|&g| {
            w.comp::<Gang>(g).and_then(|x| x.leader).is_some_and(|l| citysim::systems::contracts::free_adult(w, l))
        })
        .collect();
    assert!(gs.len() >= 2, "two led gangs");
    (gs[0], gs[1])
}

/// Parley: a Persuade on the rival leader; success pays a weekly tribute,
/// closes the vendetta and warms Regard both ways.
#[test]
fn test_parley_makes_peace_for_tribute() {
    let mut w = gang_world();
    let (g, r) = two_gangs(&mut w);
    let (l, rl) = (w.comp::<Gang>(g).unwrap().leader.unwrap(), w.comp::<Gang>(r).unwrap().leader.unwrap());
    heavy(&mut w, l);
    soft(&mut w, rl);
    w.comp_mut::<Gang>(g).unwrap().treasury = 1000;
    citysim::systems::grudges::declare(&mut w, g, r, 0.8);
    let regard = w.regard.get(&(g, r)).map_or(0.0, |x| x.value);
    // Shadow V4 review: the rival leader receives him (on the moved city
    // its standing over his, and its taste for his look, refused the
    // meeting: `moves::meeting_refused`, the gate the parley does not test).
    w.config.taste.meet_gap = f32::MAX;
    assert!(until_ok(&mut w, |w| leverage::try_parley(w, g, r)), "the parley landed");
    assert!(!w.vendettas.iter().any(|v| (v.a == g && v.b == r) || (v.a == r && v.b == g)), "the feud closed");
    // The pressure pass (phases 3 and 5a merged): the city may already hold
    // another gang pair's Parley tribute, so read this pair's.
    let c = w
        .coercions
        .values()
        .find(|c| c.why == CoercionWhy::Parley && c.holder == Some(r) && c.payer == Some(g))
        .expect("a tribute");
    assert_eq!(c.paid, 1);
    assert!(w.regard[&(g, r)].value > regard);
    assert!(has_event(&w, EventKind::Parleyed, l));
}

/// A failed Parley declares the feud for `declared_days`.
#[test]
fn test_parley_refused_declares_the_feud() {
    let mut w = gang_world();
    let (g, r) = two_gangs(&mut w);
    let (l, rl) = (w.comp::<Gang>(g).unwrap().leader.unwrap(), w.comp::<Gang>(r).unwrap().leader.unwrap());
    if let Some(s) = w.comp_mut::<Skills>(l) {
        s.persuasion = 0.0;
    }
    if let Some(p) = w.comp_mut::<Personality>(rl) {
        p.greed = 0.0;
    }
    assert!(until_ok(&mut w, |w| !leverage::try_parley(w, g, r)), "a refusal");
    assert!(w.vendettas.iter().any(|v| ((v.a == g && v.b == r) || (v.a == r && v.b == g)) && v.declared.is_some()));
}

/// A coerced job: a head who cannot pay leans on a weaker regular; the
/// record is unpaid (`Terms::Coerced`, no escrow), taken at once, and its
/// fulfilment is the coercion's compliance; no coin moves.
#[test]
fn test_coerced_job_unpaid_and_complied() {
    let mut w = gang_world();
    let (g, _) = two_gangs(&mut w);
    let head = w.comp::<Gang>(g).unwrap().leader.unwrap();
    heavy(&mut w, head);
    let fixer = contracts::open_fixers(&w)[0];
    let [gun, t] = strangers(&w, 2)[..] else { unreachable!() };
    soft(&mut w, gun);
    let now = w.tick;
    w.comp_mut::<citysim::contract::Broker>(fixer).unwrap().regulars.insert(gun, now);
    let treasury = w.purse(Some(g));
    let extra = PostExtra { deposit: None, motive: Motive::Rival, why: None };
    assert!(
        until_ok(&mut w, |w| leverage::coerced_job(
            w,
            g,
            head,
            ContractKind::Beat,
            t,
            Some(fixer),
            Origin::GangOrder(citysim::Order::Expand),
            extra.clone()
        )),
        "the threat landed"
    );
    // This post's record (the city's history can hold an earlier coerced job).
    let (&id, c) = w
        .contracts
        .iter()
        .find(|(_, c)| {
            matches!(c.terms, Terms::Coerced(_)) && c.kind == ContractKind::Beat && c.target == Target::Agent(t)
        })
        .expect("posted");
    // The strongest regular weaker than the head (the fixture's soft gun, or
    // a weaker regular already on the books).
    let taker = c.taker.expect("taken at once");
    assert!(moves::might(&w, taker) < moves::might(&w, head));
    assert_eq!((c.status, c.escrow, c.price), (ContractStatus::Taken, 0, 0));
    let Terms::Coerced(cid) = c.terms else { unreachable!() };
    contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
    assert_eq!(w.coercions[&cid].status, CoercionStatus::Complied);
    assert_eq!(w.purse(Some(g)), treasury, "nothing paid");
}

/// Fraud on a renege: a lawful taker files against the placing agent; a
/// lawless one keeps the grudge only.
#[test]
fn test_renege_files_fraud_by_a_lawful_taker() {
    for (lawful, files) in [(0.9, true), (0.1, false)] {
        let mut w = world();
        let [buyer, t, taker] = strangers(&w, 3)[..] else { unreachable!() };
        set_coins(&mut w, buyer, 1000);
        let p = Posting {
            buyer: Some(buyer),
            agent: Some(buyer),
            kind: ContractKind::Beat,
            target: Target::Agent(t),
            broker: None,
            deadline_days: 7,
            origin: Origin::God,
            price: Some(50),
        };
        let id = contracts::post(&mut w, p).expect("posted");
        w.contracts.get_mut(&id).unwrap().renege = true;
        if let Some(x) = w.comp_mut::<Personality>(taker) {
            x.lawfulness = lawful;
        }
        contracts::god_take(&mut w, id, taker).expect("taken");
        contracts::settle(&mut w, id, contracts::Settle::Fulfilled, "");
        assert_eq!(w.contracts[&id].status, ContractStatus::Reneged);
        let filed = w.crime_reports().iter().any(|r| r.crime == Crime::Fraud && r.suspect == buyer);
        assert_eq!(filed, files, "lawfulness {lawful}");
    }
}

/// The fake job: the fee changes hands; at the deadline the victim learns
/// it (`Defrauded`, a `Betrayed` grudge) and, lawful, files Fraud.
#[test]
fn test_fake_job_fee_then_the_victim_learns() {
    let mut w = world();
    let [f, v] = strangers(&w, 2)[..] else { unreachable!() };
    heavy(&mut w, f);
    soft(&mut w, v);
    if let Some(p) = w.comp_mut::<Personality>(v) {
        p.lawfulness = 0.9;
    }
    set_coins(&mut w, v, 200);
    let before = identity(&w);
    assert!(until_ok(&mut w, |w| leverage::try_fake_job(w, f, v)), "the sale landed");
    assert_eq!(w.purse(Some(v)), 200 - w.config.leverage.fake_fee);
    w.fake_jobs[0].due = w.tick;
    to_midnight(&mut w);
    assert!(has_event(&w, EventKind::Defrauded, v));
    assert!(citysim::systems::grudges::holds(&w, v, f, 0.0), "the victim holds a grudge");
    assert!(w.crime_reports().iter().any(|r| r.crime == Crime::Fraud && r.suspect == f));
    let _ = before;
}

/// The Court suitor: rejected twice by the same person, unsociable, with the
/// price: buys a `Persuade` (Origin::Court); the taker's Charm, when it
/// lands, warms the courted toward the buyer and fulfils the record.
#[test]
fn test_court_persuade_posted_and_resolved() {
    let mut w = world();
    let [s, t, taker] = strangers(&w, 3)[..] else { unreachable!() };
    if let Some(p) = w.comp_mut::<Personality>(s) {
        p.sociability = 0.1;
    }
    set_coins(&mut w, s, 5000);
    w.remember(s, MemoryKind::Rejected, Some(t), 0.6, -0.5, false);
    // Two refusals, past the memory's merge window apart.
    w.tick += 120;
    w.remember(s, MemoryKind::Rejected, Some(t), 0.6, -0.5, false);
    assert_eq!(leverage::court_target(&w, s), Some(t));
    assert!(leverage::try_court(&mut w, s, t), "posted");
    let id = *w.contracts.iter().find(|(_, c)| c.kind == ContractKind::Persuade).expect("a Persuade").0;
    assert_eq!(w.contracts[&id].origin, Origin::Court);
    heavy(&mut w, taker);
    w.config.contracts.max_attempts = 200;
    let aff = w.edge(t, s).map_or(0.0, |e| e.affinity);
    let done = until_ok(&mut w, |w| {
        if w.contracts[&id].status == ContractStatus::Open {
            contracts::god_take(w, id, taker).expect("taken");
        }
        contracts::resolve_ledger(w, id);
        w.contracts[&id].status == ContractStatus::Fulfilled
    });
    assert!(done, "the Charm landed");
    assert!(w.edge(t, s).map_or(0.0, |e| e.affinity) > aff, "warmer toward the suitor");
    assert_eq!(w.stats.current.leverage.k_persuade_done, 1);
}

/// M16b review finding 1: a weekly coercion held by a faction (a gang) is
/// paid, not broken on its first week (a gang is no living person).
#[test]
fn test_faction_held_weekly_coercion_is_paid() {
    let mut w = gang_world();
    let (g, _) = two_gangs(&mut w);
    let l = w.comp::<Gang>(g).unwrap().leader.unwrap();
    let [over] = strangers(&w, 1)[..] else { unreachable!() };
    set_coins(&mut w, over, 1_000);
    let before = w.purse(Some(g));
    let id = leverage::force(
        &mut w,
        Some(g),
        l,
        over,
        Leverage::Threat,
        Demand::Coins { amount: 50, weekly: true },
        CoercionWhy::Other,
        None,
        None,
    );
    let c = &w.coercions[&id];
    assert_eq!((c.status, c.paid), (CoercionStatus::Active, 1), "paid its first week");
    assert_eq!(w.purse(Some(g)), before + 50);
}

/// M16b review finding 3: a forced one-off sum is paid at once and closes;
/// any other forced one-off demand stands `retry_days`, then closes.
#[test]
fn test_forced_one_off_coercions_close() {
    let mut w = world();
    let [a, over, s] = strangers(&w, 3)[..] else { unreachable!() };
    set_coins(&mut w, over, 500);
    let paid = leverage::force(
        &mut w,
        Some(a),
        a,
        over,
        Leverage::Debt { amount: 80 },
        Demand::Coins { amount: 80, weekly: false },
        CoercionWhy::Other,
        None,
        None,
    );
    assert_eq!(w.coercions[&paid].status, CoercionStatus::Complied);
    assert_eq!(w.purse(Some(over)), 420);
    let now = w.tick;
    let hush = leverage::force(
        &mut w,
        Some(a),
        a,
        s,
        Leverage::Threat,
        Demand::Silence { suspect: a, tick: now },
        CoercionWhy::Other,
        None,
        None,
    );
    assert!(w.coercions[&hush].is_active());
    w.run_ticks(u64::from(w.config.leverage.retry_days) * TICKS_PER_DAY + TICKS_PER_DAY);
    assert_eq!(w.coercions.get(&hush).map(|c| c.status), Some(CoercionStatus::Complied), "its term ran");
}

/// M16b review finding 5: a closed Parley cools `(payer, holder)` and a
/// closed coerced job `(head, gun)`, the pairs their passes check.
#[test]
fn test_closed_coercions_cool_the_pair_their_pass_checks() {
    let mut w = gang_world();
    let (g, r) = two_gangs(&mut w);
    let l = w.comp::<Gang>(g).unwrap().leader.unwrap();
    w.comp_mut::<Gang>(g).unwrap().treasury = 1_000;
    let tribute = leverage::force(
        &mut w,
        Some(r),
        l,
        l,
        Leverage::Threat,
        Demand::Coins { amount: 20, weekly: true },
        CoercionWhy::Parley,
        Some(g),
        None,
    );
    let [head, gun] = strangers(&w, 2)[..] else { unreachable!() };
    let job =
        leverage::force(&mut w, Some(g), head, gun, Leverage::Threat, Demand::Do(0), CoercionWhy::Job, None, None);
    w.lev_tried.clear();
    leverage::finish(&mut w, tribute, CoercionStatus::Broken, "a test");
    leverage::finish(&mut w, job, CoercionStatus::Broken, "a test");
    assert!(leverage::tried_recently(&w, g, r, leverage::tried::PARLEY), "the Parley pair cooled");
    assert!(leverage::tried_recently(&w, head, gun, leverage::tried::JOB), "the job pair cooled");
}
