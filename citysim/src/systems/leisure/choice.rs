use super::*;

// ---------------------------------------------------------------------------
// The rung ladder (plan L14, L17)
// ---------------------------------------------------------------------------

/// A satisfier on offer to one agent.
#[derive(Clone, Debug)]
pub(super) struct Cand {
    rung: Rung,
    pub(super) act: ActionKind,
    pub(super) venue: Option<EntityId>,
    spot: Option<TilePos>,
    pub(super) score: f32,
    /// F3 (OPEN-190): the stay-in fallback's own-Home HangOut.
    stay_in: bool,
}

pub(crate) fn rung_of(world: &World, b: EntityId, price: i64) -> Rung {
    match kind_of(world, b) {
        Some(BuildingKind::Lounge) => Rung::High,
        // Jobs & lives phase 5: the Dollhouse on the high rung.
        Some(BuildingKind::Dollhouse) => Rung::High,
        Some(BuildingKind::Club) if price >= 12 => Rung::High,
        Some(BuildingKind::Club | BuildingKind::FightPit | BuildingKind::Den) => Rung::Mid,
        // Jobs & lives phase 4: the Vice venues on the mid rung; a Spire
        // Boutique on the high one.
        Some(BuildingKind::StripClub | BuildingKind::Brothel) => Rung::Mid,
        Some(BuildingKind::Boutique) => {
            if world.comp::<Building>(b).is_some_and(|bd| bd.tier >= 2) {
                Rung::High
            } else {
                Rung::Mid
            }
        }
        _ => Rung::Cheap,
    }
}

pub(super) fn act_of(kind: BuildingKind) -> ActionKind {
    match kind {
        BuildingKind::Bar => ActionKind::Drink,
        BuildingKind::NoodleBar => ActionKind::EatOut,
        BuildingKind::Den => ActionKind::Gamble,
        _ => ActionKind::Enjoy,
    }
}

/// Spec § 2: execs and the top wealth decile reach the high rung.
/// The pressure pass R6a (OPEN-217): the lonely with `[customers]
/// client_coins` reach the Dollhouse below the high rung's purse (it was
/// the execs' and the top tenth's alone: 25 visits in 30 days at 20k, the
/// joytoys' shifts empty). Nothing with `[trade_verbs]` absent.
pub(super) fn lonely_client(world: &World, id: EntityId, b: EntityId) -> bool {
    world.config.trade_verbs.is_some()
        && kind_of(world, b) == Some(BuildingKind::Dollhouse)
        && crate::systems::customers::lonely_wants(world, id)
}

pub(super) fn high_ok(world: &World, id: EntityId) -> bool {
    crate::systems::life::exec_corp(world, id).is_some()
        || (world.wealth_p90 > 0 && coins(world, id) >= world.wealth_p90)
}

/// The walk's factor (`life::travel_factor`; 1 with the life pass off).
pub(super) fn travel(world: &World, tiles: u32) -> f32 {
    crate::systems::life::travel_factor(world, crate::systems::life::walk_ticks(world, tiles))
}

/// The island pass phase 2 (`[distance.free]`): a spot's travel estimate
/// from `from`, `None` off. C9a: the body's own trip (`distance::trip`: the
/// El only when he would board it, else the walk; `travel` with
/// `[distance.choice]` off): the El's estimate let a loader walk 62 min,
/// 1,038 tiles, to an hour's HangOut under the 60-min cap.
fn spot_ticks(world: &World, id: EntityId, from: TilePos, tile: TilePos) -> Option<Tick> {
    crate::systems::distance::free(world).map(|_| crate::systems::distance::trip(world, id, from, tile))
}

/// The walk's factor to a spot at `tile`: the travel estimate's with
/// `[distance.free]` on, else the Manhattan walk's (the base).
pub(super) fn spot_travel(world: &World, id: EntityId, from: TilePos, tile: TilePos) -> f32 {
    match spot_ticks(world, id, from, tile) {
        Some(t) => crate::systems::life::travel_factor(world, t),
        None => travel(world, tile.manhattan(from)),
    }
}

/// A spot's distance for the cap and the ties: the travel estimate on, the
/// Manhattan tiles off.
pub(super) fn spot_dist(world: &World, id: EntityId, from: TilePos, tile: TilePos) -> u64 {
    spot_ticks(world, id, from, tile).unwrap_or_else(|| u64::from(tile.manhattan(from)))
}

/// The spot cap in `spot_dist`'s unit: `spot_cap_ticks` on, else
/// `[life] spot_walk_cap_tiles`.
pub(super) fn spot_cap(world: &World) -> u64 {
    crate::systems::distance::free(world).map_or(u64::from(world.config.life.spot_walk_cap_tiles), |f| f.spot_cap_ticks)
}

/// A paid candidate's score: `gain × travel × pride (mid, high) × courage
/// (FightPit, Den) × (1 − greed × price ÷ purse)`.
fn paid_score(world: &World, id: EntityId, b: EntityId, rung: Rung, price: i64, purse: i64, tiles: u32) -> f32 {
    let p = world.comp::<Personality>(id);
    let (pride, courage, greed) = p.map_or((0.5, 0.5, 0.5), |p| (p.pride, p.courage, p.greed));
    let mut s = gain_at(world, b) * travel(world, tiles);
    if matches!(rung, Rung::Mid | Rung::High) {
        s *= 1.0 + 0.5 * pride;
    }
    if matches!(kind_of(world, b), Some(BuildingKind::FightPit | BuildingKind::Den)) {
        let c = 0.5 + courage;
        s *= if courage >= world.config.leisure.gamble_rung_courage { c } else { 0.5 * c };
    }
    // Jobs & lives phase 4: a Brothel weighed by the visitor's intimacy, a
    // Boutique by its status (a branch: the older kinds score as before).
    if let Some(k) = kind_of(world, b).filter(|&k| matches!(k, BuildingKind::Brothel | BuildingKind::Boutique)) {
        s *= crate::systems::vice::score_mult(world, id, k) * crate::systems::fashion::score_mult(world, id, k);
    }
    // The pressure pass phase 2 (OPEN-176): the lonely and the miserable
    // are drawn to a venue whose companion, host or dancer is on duty.
    s *= crate::systems::customers::client_mult(world, id, b);
    let share = price.max(1) as f32 / purse.max(1) as f32;
    s * (1.0 - greed * share).max(0.05)
}

/// The kinds the ladder offers, cheap to dear (Jobs & lives phase 4's
/// three last: none stands with their switches off).
pub(super) const LADDER: [BuildingKind; 11] = [
    BuildingKind::Bar,
    BuildingKind::Arcade,
    BuildingKind::NoodleBar,
    BuildingKind::Club,
    BuildingKind::FightPit,
    BuildingKind::Den,
    BuildingKind::Lounge,
    BuildingKind::StripClub,
    BuildingKind::Brothel,
    BuildingKind::Boutique,
    BuildingKind::Dollhouse,
];

/// Where a paid visit starts from: a Full agent where it stands (`At`), or
/// a Statistical worker's day (`Day`): the Manhattan rectangle its Home's
/// door and its workplace's door span, every shortest walk between the two
/// inside it (Jobs & lives phase 7b: the Full tier's paid visits start at
/// home, at work or on the street between, 73 % of a worker's within the
/// walk cap of that rectangle on the calibration city).
#[derive(Clone, Copy, Debug)]
pub(super) enum Origin {
    At(TilePos),
    Day(TilePos, TilePos),
}

impl Origin {
    /// The origin's tile nearest `t` (the rectangle's clamp).
    pub(super) fn near(self, t: TilePos) -> TilePos {
        match self {
            Origin::At(p) => p,
            Origin::Day(a, b) => {
                TilePos { x: t.x.clamp(a.x.min(b.x), a.x.max(b.x)), y: t.y.clamp(a.y.min(b.y), a.y.max(b.y)) }
            }
        }
    }

    /// Manhattan tiles from the origin to `t`.
    pub(super) fn dist(self, t: TilePos) -> u32 {
        self.near(t).manhattan(t)
    }
}

impl Origin {
    /// The origin's box, `(lo, hi)` corners (a point for `At`).
    fn bounds(self) -> (TilePos, TilePos) {
        match self {
            Origin::At(p) => (p, p),
            Origin::Day(a, b) => {
                (TilePos { x: a.x.min(b.x), y: a.y.min(b.y) }, TilePos { x: a.x.max(b.x), y: a.y.max(b.y) })
            }
        }
    }
}

/// The perf pass phase 2 (the venue index): the venues of `kind` (those of
/// `buildings_of_kind` with a Building) handed to `test` nearest first by
/// `(door distance from `from`, id)`, up to the first that passes and no
/// further than `max_d` tiles: the sequence the old collect-and-sort of
/// every venue of the kind handed to its `find`, read off the kind's grid
/// (`World::kind_grid`) ring by ring.
pub(super) fn venue_by_distance(
    world: &World,
    kind: BuildingKind,
    from: Origin,
    max_d: u32,
    test: impl FnMut(u32, EntityId) -> bool,
) -> Option<(u32, EntityId)> {
    let (lo, hi) = from.bounds();
    world.kind_grid(kind).first_by_distance(lo, hi, max_d, |door| from.dist(door), test)
}

/// The most tiles from the origin a venue's door can be and still be within
/// `reach` ticks' walk for `id` (`exec::walk_reach_tiles`; `u32::MAX`
/// unbounded).
pub(super) fn reach_tiles(world: &World, id: EntityId, reach: Option<u64>) -> u32 {
    let limit = (world.map.w() + world.map.h()) as u32;
    reach.map_or(u32::MAX, |r| crate::exec::walk_reach_tiles(world, id, r, limit))
}

/// The nearest open venue of `kind` `id` may enter from `from` (door
/// Manhattan, ties the lower id), restricted to `district` when given, and
/// `None` past `max_d` tiles (the perf pass phase 2: the callers drop a pick
/// past the walk cap, and no door further than `max_d` is within it).
pub(super) fn nearest_venue(
    world: &World,
    id: EntityId,
    kind: BuildingKind,
    from: Origin,
    district: Option<DistrictId>,
    max_d: u32,
) -> Option<(u32, EntityId)> {
    // Scale S4 perf: the distances first, then the tests nearest first
    // until one passes (the door policy reads the staff, the room the
    // reservations), not the tests over every venue of the kind: the same
    // pick. The perf pass phase 2: the nearest first off the kind's grid.
    // Shadow V4 bug 18: not the agent's own workplace (staff spent their
    // evening at their own Brothel or pit, then waited up to 10 h there for
    // the shift).
    if door_bars_kind(world, id, kind) {
        return None;
    }
    let own = world.comp::<crate::components::Job>(id).and_then(|j| j.employer);
    venue_by_distance(world, kind, from, max_d, |_, b| {
        Some(b) != own
            && standing(world, b)
            && (kind == BuildingKind::Bar || world.comp::<Building>(b).is_some_and(|bd| bd.venue.is_some()))
            && district.is_none_or(|d| world.district_of_building(b) == d)
            && open(world, b)
            && door_seen(world, id, b)
            && has_room(world, id, b)
            // Round 6 V2 (TURF § 2): nor a venue in a contested cell after dark.
            && !world.comp::<Building>(b).is_some_and(|bd| crate::systems::turf::avoids(world, id, bd.door))
    })
}

/// The paid rungs open to `id` from `from` with `purse` to spend. Day gaps
/// 2 (2026-10-11): with `reach`, a venue whose walk (`exec::timed_walk`,
/// from the origin's nearest tile) is longer is off the ladder (the Full
/// agent's pick; since Jobs & lives phase 7b the Statistical evening's
/// too, from its day). With `district`, its venue of a kind in reach first,
/// else the nearest of the kind anywhere.
pub(super) fn paid_candidates(
    world: &World,
    id: EntityId,
    from: Origin,
    purse: i64,
    district: Option<DistrictId>,
    reach: Option<u64>,
) -> Vec<Cand> {
    let mut out = Vec::new();
    if purse < 1 {
        return out;
    }
    let drank = crate::utility::goals::drank_today(world, id);
    let hungry = world.comp::<Needs>(id).is_some_and(|n| n.hunger < 0.5);
    let high = high_ok(world, id);
    let max_d = reach_tiles(world, id, reach);
    for kind in LADDER {
        if kind == BuildingKind::Bar && drank {
            continue;
        }
        if kind == BuildingKind::NoodleBar && !hungry {
            continue;
        }
        let in_reach = |&(_, b): &(u32, EntityId)| {
            reach.is_none_or(|r| {
                world
                    .comp::<Building>(b)
                    .is_some_and(|bd| crate::exec::timed_walk(world, id, from.near(bd.door), bd.door) <= r)
            })
        };
        let pick = nearest_venue(world, id, kind, from, district, max_d)
            .filter(in_reach)
            .or_else(|| district.and_then(|_| nearest_venue(world, id, kind, from, None, max_d)));
        let Some((tiles, b)) = pick.filter(in_reach) else { continue };
        let price = price_of(world, b);
        // The rung's affordability (spec § 2's brake): at most half the purse above
        // the reserve goes on one evening.
        if price <= 0 || 2 * price > purse {
            continue;
        }
        if kind == BuildingKind::NoodleBar && world.comp::<Building>(b).is_none_or(|bd| bd.stock_food == 0) {
            continue;
        }
        if kind == BuildingKind::Den && !table_open(world, b) {
            continue;
        }
        let rung = rung_of(world, b, price);
        if rung == Rung::High && !high && !lonely_client(world, id, b) {
            continue;
        }
        let score = paid_score(world, id, b, rung, price, purse, tiles);
        out.push(Cand { rung, act: act_of(kind), venue: Some(b), spot: None, score, stay_in: false });
    }
    out
}

/// The free show: a live raid's door or a FightPit's bout within
/// `watch_tiles` of `from` (the street outside it).
fn watch_tile(world: &World, from: TilePos) -> Option<TilePos> {
    let reach = world.config.leisure.watch_tiles;
    let mut best: Option<(u32, TilePos)> = None;
    let mut consider = |t: TilePos| {
        let d = t.manhattan(from);
        if d <= reach && best.is_none_or(|(bd, bt)| (d, t) < (bd, bt)) {
            best = Some((d, t));
        }
    };
    for &g in world.gang_list() {
        if world.comp::<Gang>(g).is_some_and(|x| x.raid_at.is_some()) {
            if let Some(t) = crate::systems::raid::gang_target(world, g)
                .and_then(|b| world.comp::<Building>(b))
                .map(|bd| world.outside_door(bd))
            {
                consider(t);
            }
        }
    }
    let hour = world.tick_of_day() / TICKS_PER_HOUR as u16;
    if hour + 1 >= world.config.leisure.bout_hour && hour <= world.config.leisure.bout_hour {
        for &b in world.buildings_of_kind(BuildingKind::FightPit) {
            if open(world, b) {
                if let Some(bd) = world.comp::<Building>(b) {
                    consider(world.outside_door(bd));
                }
            }
        }
    }
    best.map(|(_, t)| t)
}

/// The walk origin of `id` (outside its building, else its tile).
pub(super) fn origin(world: &World, id: EntityId) -> Option<TilePos> {
    crate::exec::walk_origin(world, id)
}

/// Plan L14: the rung `id` would take now, scored as Unwind scores it:
/// the paid venues (nearest open of each kind), the free show and the best
/// HangOut spot. `scored` runs the full spot scoring (`hunt::habit` over
/// the contacts, at plan time); unscored picks the nearest spot (think).
pub fn choice(world: &World, id: EntityId, scored: bool) -> Option<UnwindPick> {
    let from = origin(world, id)?;
    // Round 8 X2 (`OPEN-247`): a funeral he is asked to is the evening; a
    // mourner or a man lying low keeps to his Home.
    if let Some((h, door)) = crate::systems::grief::wake_pick(world, id) {
        let score =
            world.config.leisure.gain.hangout_hour * world.config.leisure.hangout_ticks as f32 / TICKS_PER_HOUR as f32;
        return Some(UnwindPick {
            rung: Rung::Free,
            act: ActionKind::HangOut,
            venue: Some(h),
            spot: Some(door),
            score,
            stay_in: false,
        });
    }
    if crate::systems::grief::holed_up(world, id) {
        let (h, door) = home_of(world, id)
            .and_then(|h| world.comp::<Building>(h).filter(|b| !b.demolished).map(|b| (h, world.outside_door(b))))?;
        let hours = world.config.leisure.hangout_ticks as f32 / TICKS_PER_HOUR as f32;
        let score = crate::systems::distance::stay_in(world)
            .map_or(world.config.leisure.gain.hangout_hour, |c| c.fun_hour)
            * hours;
        return Some(UnwindPick {
            rung: Rung::Free,
            act: ActionKind::HangOut,
            venue: Some(h),
            spot: Some(door),
            score,
            stay_in: true,
        });
    }
    let purse = spendable(world, id);
    let reach = world.config.leisure.venue_walk_cap_ticks;
    let mut cands = paid_candidates(world, id, Origin::At(from), purse, None, Some(reach));
    // Spec § 2 (stat_daily's rule, kept on screen for parity): a hungry
    // agent's evening is the NoodleBar's meal when one is in reach.
    if let Some(c) = cands.iter().find(|c| c.act == ActionKind::EatOut) {
        return Some(pick_of(c));
    }
    let g = &world.config.leisure.gain;
    if let Some(t) = watch_tile(world, from) {
        let score = g.watch * travel(world, t.manhattan(from));
        cands.push(Cand {
            rung: Rung::Free,
            act: ActionKind::HangOut,
            venue: None,
            spot: Some(t),
            score,
            stay_in: false,
        });
    }
    // Lives at work phase 2 (§ 3.1): a Home spot carries its Home as the
    // pick's venue (the HangOut runs inside it).
    let (spot, past_cap) = if scored {
        let (best, past) = spot_pick(world, id, from);
        let best = best.map(|(s, social)| {
            let home = match s.kind {
                SpotKind::Home(h) => Some(h),
                _ => None,
            };
            (s.tile, social, home, false)
        });
        (best, past)
    } else {
        // The island pass phase 2: on, the nearest is `None` only with no
        // spot within the cap.
        let near = nearest_spot(world, id, from).map(|(t, social)| (t, social, None, false));
        let past = near.is_none();
        (near, past)
    };
    // Island fix round F3 (OPEN-190): with no spot within the cap, the
    // stay-in: the own Home's HangOut, scored at `stay_in.fun_hour`.
    let spot = spot.or_else(|| {
        if !past_cap {
            return None;
        }
        stay_in_home(world, id).map(|(h, t)| (t, 0.0, Some(h), true))
    });
    if let Some((t, social, home, stay_in)) = spot {
        let hours = world.config.leisure.hangout_ticks as f32 / TICKS_PER_HOUR as f32;
        // L2 shadow fixes item 5: a lone hour is not a full hour's fun (11
        // of 15 HangOuts were "nobody there"); company scales it up.
        let lone = world.config.life.spot_lone_fun;
        let company = lone + (1.0 - lone) * social.clamp(0.0, 1.0);
        let fun = match crate::systems::distance::stay_in(world).filter(|_| stay_in) {
            Some(c) => c.fun_hour * hours,
            None => g.hangout_hour * hours * company,
        };
        let score = (fun + 0.1 * social.min(1.0)) * spot_travel(world, id, from, t);
        cands.push(Cand { rung: Rung::Free, act: ActionKind::HangOut, venue: home, spot: Some(t), score, stay_in });
    }
    let best = cands.iter().max_by(|a, b| {
        a.score.total_cmp(&b.score).then_with(|| b.venue.cmp(&a.venue)).then_with(|| b.spot.cmp(&a.spot))
    })?;
    Some(pick_of(best))
}

fn pick_of(c: &Cand) -> UnwindPick {
    UnwindPick { rung: c.rung, act: c.act, venue: c.venue, spot: c.spot, score: c.score, stay_in: c.stay_in }
}
