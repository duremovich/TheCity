use super::*;

/// Items 8-10: where `id`'s working day is spent, the point every commute
/// rule measures to: its employer's door, except a sweeper's (Sweep on),
/// which is its beat district's centroid (the Recycler is only its wage
/// desk; the behaviour tier's d19 sweepers walked 10.25 h a day, 7.43 with
/// items 8-10 off, housed toward the Recycler and away from their beats).
pub fn work_tile(world: &World, id: EntityId, employer: EntityId) -> Option<TilePos> {
    // The pre-view pass phase 4 (P8): the sweeper's round ends at the
    // Recycler (the post), the point a relocation anchors to (the beat is
    // re-dealt nightly: 6-8 of 15 sweeper couples re-housed "nearer work").
    if crate::systems::jobs::is_sweeper(world, id) && world.config.presence.is_none() {
        if let Some(c) =
            crate::systems::jobs::beat_of(world, id).and_then(|d| world.districts.get(d.index())).map(|x| x.centroid)
        {
            return Some(c);
        }
    }
    world.comp::<Building>(employer).filter(|b| !b.demolished).map(|b| b.door)
}

/// Daily: up to `relocate_per_day` employed adults whose commute is past
/// `commute_cap_tiles` move (with a spouse and children who share their
/// Home) into the Home with room nearest their workplace that they can pay
/// `[rent] rehouse_coins_mult` x rent for, when it is nearer by the margin.
/// Longest commutes first (ties lower id). A guard's workplace is the Precinct.
/// Hunger fix item 8 (P5g): with the fix, the daily cap is
/// `relocate_per_1000` per 1,000 residents (`relocate_up_to` has the rule).
pub fn relocate(world: &mut World) {
    let limit = if crate::systems::fixes::relocate_fix(world) {
        let pop = world.citizens().len() as f32;
        (world.config.life.relocate_per_1000 * pop / 1000.0).round().max(world.config.life.relocate_per_day as f32)
            as usize
    } else {
        world.config.life.relocate_per_day
    };
    relocate_up_to_with(world, limit, true, true);
}

/// `relocate` with a mover limit (world creation: uncapped). Hunger fix item
/// 8 (P5g): a mover is a worker whose `commute_cost` is past
/// `commute_move_ticks`; the destination is the nearest Home with room
/// within that of work (else a trade), paid by the first rent (no
/// `rehouse_coins_mult` deposit), saving `commute_margin_ticks`. Off, the
/// L1 rule in tiles (`commute_cap_tiles`, `bed_margin_tiles`).
pub fn relocate_up_to(world: &mut World, limit: usize) {
    relocate_up_to_with(world, limit, !crate::spatial::scans(), false);
}

/// [`relocate_up_to`]; `lazy` false fills every Home's measure to a work
/// tile before the pick's filters (the pre-review walk, for the tests).
/// `daily` (the midnight pass, not world creation): with `[commute_pressure]`
/// on, the island pass phase 4's rule (`systems/commute.rs`): a mover is a
/// worker whose far commute has built `move_days` of pressure (or a far hire
/// since the last midnight); the destination is a Home within `[distance]
/// home_cap_ticks` of work or a company bed of his corp; a household with
/// a School near moves only near one; with none found, past `job_days`, a
/// nearer post.
pub(super) fn relocate_up_to_with(world: &mut World, limit: usize, lazy: bool, daily: bool) {
    let _tp = crate::prof::scope!("reloc::pass");
    let fix = crate::systems::fixes::relocate_fix(world);
    let pressure = daily && crate::systems::commute::on(world);
    let mut read: std::collections::BTreeSet<EntityId> = std::collections::BTreeSet::new();
    // The measure: travel ticks with the fix, the L1 tiles without.
    let cost = |w: &World, a: TilePos, b: TilePos| -> u64 {
        if fix {
            commute_cost(w, a, b)
        } else {
            u64::from(a.manhattan(b))
        }
    };
    let cap: u64 =
        if fix { world.config.life.commute_move_ticks } else { u64::from(world.config.life.commute_cap_tiles) };
    let margin: u64 =
        if fix { world.config.life.commute_margin_ticks } else { u64::from(world.config.life.bed_margin_tiles) };
    // The island pass phase 4: the destination's cap.
    let dest_cap = if pressure { crate::systems::commute::home_cap(world) } else { cap };
    let mult = if fix { 1 } else { world.config.rent.rehouse_coins_mult };
    let mut movers: Vec<(std::cmp::Reverse<u64>, EntityId, EntityId, TilePos)> = Vec::new();
    // scan-ok: daily: the relocation pass
    // An exec's workplace is its corp's HQ.
    let execs: std::collections::BTreeMap<EntityId, EntityId> = world
        .corps()
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).and_then(|cc| cc.exec).map(|e| (e, c)))
        .collect();
    // Round 9 Y4: the read's commutes, for the day's median (a print).
    let mut commutes: Vec<u64> = Vec::new();
    {
        let _scan = crate::prof::scope!("reloc::scan");
        for a in world.citizens() {
            let employer = match world.comp::<Job>(a) {
                Some(job) => job.employer,
                None => execs.get(&a).and_then(|&c| hq_of(world, c, a)),
            };
            let Some(employer) = employer else { continue };
            if world.has::<Sentence>(a) || world.comp::<Brain>(a).is_none_or(|b| b.emigrating) {
                continue;
            }
            let Some(home) = home_of(world, a) else { continue };
            // OPEN-140: one move a body a midnight; an elder under a grown
            // child's roof stays there (the kin take-in moved him back).
            if housed_now(world, a) || crate::systems::ages::with_kin(world, a) {
                continue;
            }
            // Lives at work phase 3 (§ 2.2, tied housing): a household housed
            // rent-free by a tied worker's landlord stays (the first build moved
            // the super's spouse nearer her work every midnight, the super with
            // her, and `tied_housing` moved them back: 50 moves in 120 days).
            if world.residents_of(home).iter().any(|&r| crate::systems::ownership::tied_here(world, r, home)) {
                continue;
            }
            // The island pass phase 3a (I6): a company block's household stays
            // (housed by the job; its own pass moves it). Phase 4: this is that
            // pass, and the household is read as any other.
            if !pressure && !crate::systems::company::open_to_all(world, home) {
                continue;
            }
            let Some(work) =
                (if fix { work_tile(world, a, employer) } else { world.comp::<Building>(employer).map(|b| b.door) })
            else {
                continue;
            };
            let Some(hd) = world.comp::<Building>(home).map(|b| b.door) else { continue };
            let d = cost(world, hd, work);
            if daily {
                commutes.push(d);
            }
            // Round 6 V4 (OPEN-240): the report's long commutes.
            if d > world.config.life.commute_long_ticks {
                crate::systems::ownership::jobs2_row(world).commute_long += 1;
            }
            if pressure {
                // The island pass phase 4: the day's pressure decides.
                read.insert(a);
                if crate::systems::commute::note(world, a, d) {
                    movers.push((std::cmp::Reverse(d), a, home, work));
                }
            } else if d > cap {
                movers.push((std::cmp::Reverse(d), a, home, work));
            }
        }
    }
    if !commutes.is_empty() {
        let mid = commutes.len() / 2;
        let (_, &mut med, _) = commutes.select_nth_unstable(mid);
        crate::systems::ownership::jobs2_row(world).commute_med = u32::try_from(med).unwrap_or(u32::MAX);
    }
    movers.sort_unstable();
    // The island pass phase 4: the company beds standing (the pick takes
    // those of the mover's corp, `commute::company_for`).
    let company: Vec<(EntityId, TilePos, usize)> = if pressure {
        world
            .company_homes
            .keys()
            .filter_map(|&h| {
                world
                    .comp::<Building>(h)
                    .filter(|b| !b.demolished && !b.derelict)
                    .map(|b| (h, b.door, usize::from(b.capacity)))
            })
            .collect()
    } else {
        Vec::new()
    };
    let homes: Vec<(EntityId, TilePos, usize)> = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        // The island pass phase 3a (I6): a company bed is its staff's only.
        .filter(|&&h| crate::systems::company::open_to_all(world, h))
        .filter_map(|&h| {
            world
                .comp::<Building>(h)
                .filter(|b| !b.demolished && !b.derelict)
                .map(|b| (h, b.door, usize::from(b.capacity)))
        })
        .collect();
    // Scale S1: each Home's measure to a work tile, once a pass per tile (the
    // pick and the trade read it for every mover; a pure function of the two
    // doors and the network, which a pass does not change).
    let mut memo = DoorCosts::new(homes.iter().map(|&(_, door, _)| door).collect());
    // The perf pass phase 3: with the fix, a Home past `dest_cap` (the pick)
    // or `cap` (the trade) of work is never taken, so each reads only the
    // Homes within the larger of the two of its work tile (`HomeReach`, a
    // superset by door and the El, cut by the measure), not every Home.
    let mut reach = (lazy && fix).then(|| HomeReach::new(world, &homes, cap.max(dest_cap)));
    let indexed = reach.is_some();
    let every: Vec<u32> = if indexed { Vec::new() } else { (0..homes.len() as u32).collect() };
    let mut moved = 0usize;
    let tick = world.tick;
    for (std::cmp::Reverse(d), a, home, work) in movers {
        if moved >= limit {
            break;
        }
        // Moved already as someone's spouse this pass.
        if home_of(world, a) != Some(home) {
            continue;
        }
        let spouse = world.spouse_of(a).filter(|&s| home_of(world, s) == Some(home) && world.has::<Brain>(s));
        // OPEN-140: a spouse the midnight housed already holds the couple.
        if spouse.is_some_and(|s| housed_now(world, s)) {
            continue;
        }
        // A spouse with a job of their own stays put (two commutes, one
        // Home). Hunger fix item 8 (P5h): with the fix the couple moves when the two
        // commutes together shrink by the margin (39 of the 42 far workers
        // left at a Farm the new Homes served on day 8 had a working spouse).
        let spouse_work = spouse
            .and_then(|s| world.comp::<Job>(s).map(|j| (s, j.employer)))
            .map(|(s, e)| e.and_then(|e| work_tile(world, s, e)));
        let spouse_work = match spouse_work {
            Some(None) => continue,
            Some(Some(_)) if !fix => continue,
            Some(Some(sw)) => Some(sw),
            None => None,
        };
        let hd = world.comp::<Building>(home).map(|b| b.door).unwrap_or(work);
        let both = |door: TilePos| spouse_work.map_or(0, |sw| cost(world, door, sw));
        let d_pair = d + both(hd);
        let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, a)
            .into_iter()
            .filter(|&c| world.has::<crate::components::Child>(c) && home_of(world, c) == Some(home))
            .collect();
        let need = 1 + usize::from(spouse.is_some()) + kids.len();
        let purse = coins(world, a);
        let evicted_by = world.comp::<Household>(a).and_then(|h| h.evicted_by).map(|(o, _)| o);
        // Class pay: a graded post holder or an exec keeps its tier.
        let mut min_tier = crate::systems::classpay::min_home_tier(world, a, home);
        // Lives at work phase 7 (§ 5.3): an exec, or an exec's spouse (who
        // moves the exec along), only to `exec_home_tier` or better (the
        // first build moved 20k execs out of the Spire as a mover's spouse).
        if execs.contains_key(&a) || spouse.is_some_and(|s| execs.contains_key(&s)) {
            min_tier = min_tier.max(exec_home_tier(world));
        }
        if !lazy {
            memo.fill(world, work, &cost);
            if let Some(sw) = spouse_work {
                memo.fill(world, sw, &cost);
            }
        }
        // Review finding 9: the room, purse, tier and evictor filters first;
        // a Home's measure only for the Homes they leave (memoised by work
        // tile and Home), the work leg before the spouse's. The same pick:
        // the filters and the minimum are the eager walk's.
        // The island pass phase 4: the children keep a School in reach.
        let school = |to: TilePos| !pressure || kids.is_empty() || crate::systems::commute::school_kept(world, hd, to);
        let near: &[u32] = match reach.as_mut() {
            Some(r) => r.of(world, work, &mut memo, &cost),
            None => &every,
        };
        let pick = crate::prof::timed!("reloc::pick", {
            let mut best: Option<(u64, EntityId)> = None;
            for &i in near {
                let i = i as usize;
                let (h, hdoor, room) = homes[i];
                let open = h != home
                    && (min_tier == 0 || world.comp::<Building>(h).is_some_and(|b| b.tier >= min_tier))
                    && world.residents_of(h).len() + need <= room
                    && purse >= mult * crate::systems::ownership::rent_for(world, h)
                    && evicted_by.is_none_or(|o| o != world.owner_of(h));
                if !open {
                    continue;
                }
                let dd = memo.cost(world, work, i, &cost);
                if (fix && dd > dest_cap) || dd + margin > d_pair {
                    continue;
                }
                // OPEN-140: "nearer work" is nearer the mover's own work
                // (the pair's sum let his leg grow while hers shrank).
                if dd + margin > d {
                    continue;
                }
                let sp = spouse_work.map_or(0, |sw| memo.cost(world, sw, i, &cost));
                if dd + sp + margin > d_pair {
                    continue;
                }
                if !school(hdoor) {
                    continue;
                }
                // Pressure pass round 6, V2 (TURF § 2): a family with children
                // moves into no contested cell.
                if !kids.is_empty()
                    && matches!(
                        crate::systems::turf::status_at(world, hdoor),
                        crate::systems::turf::Status::Contested(..)
                    )
                {
                    continue;
                }
                if best.is_none_or(|b| (dd + sp, h) < b) {
                    best = Some((dd + sp, h));
                }
            }
            // The island pass phase 4: the mover's corp's company beds, on
            // the same rules (no first rent: it is docked from the wage).
            for &(h, hdoor, room) in &company {
                if h == home
                    || !crate::systems::commute::company_for(world, a, h)
                    || world.residents_of(h).len() + need > room
                    || (min_tier != 0 && world.comp::<Building>(h).is_none_or(|b| b.tier < min_tier))
                {
                    continue;
                }
                let dd = cost(world, hdoor, work);
                let sp = spouse_work.map_or(0, |sw| cost(world, hdoor, sw));
                if dd > dest_cap || dd + margin > d || dd + sp + margin > d_pair || !school(hdoor) {
                    continue;
                }
                if best.is_none_or(|b| (dd + sp, h) < b) {
                    best = Some((dd + sp, h));
                }
            }
            best
        });
        // Hunger fix item 8 (P5g): no Home with room near work (the Farms'
        // neighbourhoods are full: 282 beds within 40 tiles of the 12 Farms
        // for 504 farmers on the flipped seed 42): trade Homes with a single
        // resident there whose own day the move does not lengthen past the
        // cap (no job, or a job no farther from the mover's Home), and who
        // can pay the mover's Home's first rent.
        let pick = match pick {
            Some(p) => Some(p),
            // The island fix round F2 (OPEN-189): a company bed is never
            // traded to the other side.
            None if fix && spouse.is_none() && kids.is_empty() && crate::systems::beds::trade_ok(world, home) => {
                let mover = Mover { purse, evicted_by, min_tier };
                // The trade walks the Homes nearest first: the whole row (the
                // perf pass phase 3: the reach, nearest first, which holds
                // every Home within `cap` the walk reads before it stops).
                let near = if indexed {
                    NearHomes { homes: &homes, to_work: memo.row(work), order: near }
                } else {
                    memo.fill(world, work, &cost);
                    NearHomes { homes: &homes, to_work: memo.row(work), order: memo.order(work) }
                };
                if let Some((_, to, r)) =
                    crate::prof::timed!("reloc::swap", swap_partner(world, a, home, near, cap, d, margin, mover))
                        .filter(|&(_, _, r)| !housed_now(world, r))
                {
                    let (an, rn) = (world.name_of(a), world.name_of(r));
                    let (to_name, from_name) = (world.name_of(to), world.name_of(home));
                    world.set_home(a, Some(to));
                    world.set_home(r, Some(home));
                    for m in [a, r] {
                        if let Some(h) = world.comp_mut::<Household>(m) {
                            h.rent_due = h.rent_due.min(0.0);
                        }
                    }
                    world.push_event(
                        EventKind::Housed,
                        &[a, to],
                        format!("{an} traded Homes with {rn} to live nearer work, into {to_name}"),
                    );
                    world.push_event(
                        EventKind::Housed,
                        &[r, home],
                        format!("{rn} traded Homes with {an}, into {from_name}"),
                    );
                    moved += 1;
                    if pressure {
                        crate::systems::commute::moved(world, a);
                        let row = crate::systems::ownership::jobs2_row(world);
                        row.pressure_moved += 1;
                        row.pressure_movers += 1;
                    }
                } else if pressure {
                    crate::systems::commute::found_nothing(world, a);
                }
                continue;
            }
            None => None,
        };
        let Some((_, to)) = pick else {
            // The island pass phase 4: nothing found; past `job_days`, a
            // nearer post at the next midnight's job search.
            if pressure {
                crate::systems::commute::found_nothing(world, a);
            }
            continue;
        };
        let company_bed = world.company_homes.contains_key(&to);
        let place = world.name_of(to);
        let mover_name = world.name_of(a);
        for m in std::iter::once(a).chain(spouse) {
            world.set_home(m, Some(to));
            // 8d (OPEN-126): what is owed is forgiven, a move-in's prepaid
            // rent (below 0) moves with the mover (it was wiped when the
            // same midnight's relocation followed the deposit).
            if let Some(h) = world.comp_mut::<Household>(m) {
                h.rent_due = h.rent_due.min(0.0);
                // The island pass phase 4: a company bed starts clean, as
                // `company::fill`'s.
                if company_bed {
                    h.arrears = 0;
                }
            }
            if pressure {
                crate::systems::commute::moved(world, m);
            }
            let name = world.name_of(m);
            // OPEN-140: "nearer work" is the worker's line; a spouse moves
            // with him (a jobless spouse read "moved nearer work").
            let text = if m != a {
                format!("{name} moved with {mover_name}, into {place}")
            } else if company_bed {
                // The island pass phase 4: 3a's company line.
                let what = crate::systems::company::of(world, to).map(|w| world.name_of(w)).unwrap_or_default();
                let label = world.owner_label(world.owner_of(to));
                format!("{name} moved nearer work, into {place}, {label}'s company housing for {what}'s staff")
            } else {
                format!("{name} moved nearer work, into {place}")
            };
            world.push_event(EventKind::Housed, &[m, to], text);
        }
        let door = world.comp::<Building>(to).map(|b| b.door).unwrap_or_default();
        let kids_n = kids.len() as u32;
        for c in kids {
            world.set_home(c, Some(to));
            world.remove_from_building(c);
            if let Some(p) = world.comp_mut::<Position>(c) {
                p.tile = door;
                p.building = None;
                p.floor = 0;
                p.entered = tick;
            }
            world.enter_building(c, to);
        }
        if pressure {
            let n = 1 + u32::from(spouse.is_some()) + kids_n;
            let row = crate::systems::ownership::jobs2_row(world);
            row.pressure_moved += 1;
            row.pressure_movers += n;
            if company_bed {
                row.pressure_company += 1;
                row.company_moved_in += 1;
            }
        }
        moved += 1;
    }
    if pressure {
        crate::systems::commute::prune(world, &read);
    }
}

/// Scale S1: per work tile, the measure from each Home door of a
/// relocation pass's list to it (`rows[tile].0[i]` for `doors[i]`) and the
/// list's indices nearest first (ties in list order). Review finding 9: a
/// row fills lazily, a Home at a time ([`DoorCosts::cost`]); `fill`
/// completes it and sorts the order (the trade's walk).
struct DoorCosts {
    doors: Vec<TilePos>,
    /// `UNKNOWN` until measured.
    rows: std::collections::BTreeMap<TilePos, (Vec<u64>, Vec<u32>)>,
}

/// A [`DoorCosts`] cell not measured yet (a real measure of `u64::MAX` is
/// measured again on each read: the same answer).
const UNKNOWN: u64 = u64::MAX;

impl DoorCosts {
    fn new(doors: Vec<TilePos>) -> DoorCosts {
        DoorCosts { doors, rows: std::collections::BTreeMap::new() }
    }

    /// The measure from Home `i`'s door to `to`, memoised.
    fn cost(&mut self, world: &World, to: TilePos, i: usize, cost: &impl Fn(&World, TilePos, TilePos) -> u64) -> u64 {
        let n = self.doors.len();
        let row = &mut self.rows.entry(to).or_insert_with(|| (vec![UNKNOWN; n], Vec::new())).0;
        if row[i] == UNKNOWN {
            row[i] = cost(world, self.doors[i], to);
        }
        row[i]
    }

    /// Every Home's measure to `to` and the order, nearest first.
    fn fill(&mut self, world: &World, to: TilePos, cost: &impl Fn(&World, TilePos, TilePos) -> u64) {
        let n = self.doors.len();
        let (row, order) = self.rows.entry(to).or_insert_with(|| (vec![UNKNOWN; n], Vec::new()));
        if !order.is_empty() || n == 0 {
            return;
        }
        for (i, c) in row.iter_mut().enumerate() {
            if *c == UNKNOWN {
                *c = cost(world, self.doors[i], to);
            }
        }
        *order = (0..n as u32).collect();
        order.sort_unstable_by_key(|&i| (row[i as usize], i));
    }

    /// The row `fill` made for `to` (empty if none).
    fn row(&self, to: TilePos) -> &[u64] {
        self.rows.get(&to).map_or(&[], |r| r.0.as_slice())
    }

    /// The row's indices, nearest first (empty if none).
    fn order(&self, to: TilePos) -> &[u32] {
        self.rows.get(&to).map_or(&[], |r| r.1.as_slice())
    }
}

/// The perf pass phase 3 (the world build): a relocation pass's Homes
/// within `limit` (`commute_cost`) of each work tile, by index into the
/// pass's list, nearest first (ties the lower index: [`DoorCosts::order`]
/// cut at `limit`). A grid of the Homes' doors queried over
/// `transit::reach_discs` (a superset), then the exact measure. The pass's
/// list and the network do not change during it, so a tile's reach is
/// found once.
struct HomeReach {
    grid: crate::spatial::TileGrid,
    limit: Tick,
    lists: std::collections::BTreeMap<TilePos, Vec<u32>>,
}

impl HomeReach {
    fn new(world: &World, homes: &[(EntityId, TilePos, usize)], limit: Tick) -> HomeReach {
        let grid = crate::spatial::TileGrid::build(
            world.map.w(),
            world.map.h(),
            crate::spatial::BUILDING_CELL,
            homes.iter().map(|&(_, door, _)| door),
        );
        HomeReach { grid, limit, lists: std::collections::BTreeMap::new() }
    }

    /// The Homes within the limit of `work`, nearest first.
    fn of(
        &mut self,
        world: &World,
        work: TilePos,
        memo: &mut DoorCosts,
        cost: &impl Fn(&World, TilePos, TilePos) -> u64,
    ) -> &[u32] {
        let (grid, limit) = (&self.grid, self.limit);
        self.lists.entry(work).or_insert_with(|| {
            let mut idx = grid.near_discs(&crate::systems::transit::reach_discs(world, work, limit));
            let mut keyed: Vec<(u64, u32)> = idx
                .drain(..)
                .filter_map(|i| {
                    let c = memo.cost(world, work, i as usize, cost);
                    (c <= limit).then_some((c, i))
                })
                .collect();
            keyed.sort_unstable();
            keyed.into_iter().map(|(_, i)| i).collect()
        })
    }
}

/// Scale S1: a relocation pass's standing Homes (`(Home, door, capacity)`),
/// each one's measure to the mover's work, and their indices nearest first.
struct NearHomes<'a> {
    homes: &'a [(EntityId, TilePos, usize)],
    to_work: &'a [u64],
    order: &'a [u32],
}

/// The mover's side of a trade (`swap_partner`), as the plain pick reads
/// it: its purse (the first rent of the Home it takes), the landlord that
/// evicted it (never traded back into), and its tier floor (class pay).
struct Mover {
    purse: i64,
    evicted_by: Option<Option<EntityId>>,
    min_tier: u8,
}

/// Hunger fix item 8 (P5g): the swap for mover `a` (Home `home`, work door
/// `work`, commute `d`): the nearest Home within `cap` of work, the mover
/// can pay the first rent of, not its evictor's and at its tier floor,
/// holding a single adult resident (no spouse or child there) who is not
/// itself a worker nearer that Home than `home`, can pay `home`'s first
/// rent, keeps its own tier floor in `home` and was not evicted by `home`'s
/// landlord. `(distance, that Home, the resident)`. Scale S1: `near` is the
/// pass's standing Homes (`(Home, door, capacity)`, the Homes of the kind
/// list that are neither demolished nor derelict, in its order) with each
/// one's `commute_cost` to the mover's work and the list's indices nearest
/// first: the first Home in that order with a partner is the nearest (ties
/// in list order), and the walk stops past `cap` or the margin.
#[allow(clippy::too_many_arguments)]
fn swap_partner(
    world: &World,
    a: EntityId,
    home: EntityId,
    near: NearHomes<'_>,
    cap: u64,
    d: u64,
    margin: u64,
    mover: Mover,
) -> Option<(u64, EntityId, EntityId)> {
    let min_tier = mover.min_tier;
    let home_door = world.comp::<Building>(home)?.door;
    let home_tier = world.comp::<Building>(home)?.tier;
    let home_owner = world.owner_of(home);
    let home_rent = crate::systems::ownership::rent_for(world, home);
    let NearHomes { homes, to_work, order } = near;
    for &i in order {
        let (h, dd) = (homes[i as usize].0, to_work[i as usize]);
        // Nearest first: every Home after this one is as far or farther.
        if dd > cap || dd + margin > d {
            break;
        }
        let Some(b) = world.comp::<Building>(h).filter(|b| b.tier >= min_tier) else {
            continue;
        };
        if h == home {
            continue;
        }
        // The mover's checks, as the plain pick's.
        if mover.purse < crate::systems::ownership::rent_for(world, h)
            || mover.evicted_by.is_some_and(|o| o == world.owner_of(h))
        {
            continue;
        }
        let residents = world.residents_of(h);
        for &r in residents.iter() {
            if r == a || !world.has::<Brain>(r) || !crate::systems::demography::is_adult(world, r) {
                continue;
            }
            if world.spouse_of(r).is_some()
                || crate::systems::demography::children_of_agent(world, r).iter().any(|&c| home_of(world, c) == Some(h))
            {
                continue;
            }
            if world.has::<Sentence>(r) || world.comp::<Brain>(r).is_some_and(|br| br.emigrating) {
                continue;
            }
            // Its own day no worse: no job, or its work no nearer `h` than `home`.
            let ok = match world.comp::<Job>(r).and_then(|j| j.employer).and_then(|e| work_tile(world, r, e)) {
                None => true,
                Some(wt) => commute_cost(world, home_door, wt) <= commute_cost(world, b.door, wt),
            };
            if !ok || coins(world, r) < home_rent {
                continue;
            }
            // The partner's tier floor and evictor, as its own pick's (Lives
            // at work phase 7: an exec's floor is `exec_home_tier`).
            let exec_floor =
                if exec_home_tier(world) > 0 && exec_corp(world, r).is_some() { exec_home_tier(world) } else { 0 };
            if crate::systems::classpay::min_home_tier(world, r, h).max(exec_floor) > home_tier
                || world.comp::<Household>(r).and_then(|x| x.evicted_by).is_some_and(|(o, _)| o == home_owner)
            {
                continue;
            }
            return Some((dd, h, r));
        }
    }
    None
}
