//! The island pass phase 3a (docs/ISLAND_PASS.md I6; OPEN-163): seeded
//! company housing.
//!
//! The island's 4-hour commute tail is the layout, not the pick: its 120
//! Farms hold 3,473 Farmer posts with 2,525 adults within 60 tiles, and
//! only 568 of its 4,332 far commuters have a free bed near work (phase
//! 1). Farms and industry house their own staff.
//!
//! - **Seeded** ([`seed`], world build, after the staffing and before the
//!   housing pre-roll): each workplace of a `[company_housing] kinds` kind
//!   whose far posts (a holder homeless, or housed past `far_ticks` of
//!   `distance::travel`) outnumber the free beds within `far_ticks` of its
//!   door wants `seed_share` of the difference as company beds (most wanted
//!   first, ties lower id; at least `min_beds`). Up to `blocks_max` blocks
//!   go up for it, each a plain Home owned by the workplace's owner: on the
//!   nearest plain vacant Lot within `lot_cap_ticks` (a shanty's is left to
//!   its squatters), else on the nearest `plot_w` x `plot_h` plot of open
//!   ground within it (every tile `Ground`, clear of any building by a tile,
//!   the door toward the workplace; a Lot record spawned there). A block
//!   holds `beds_per_floor` bunks a floor (the interior allowing) and the
//!   storeys its want asks for, at most `floors_max`. The island's Farm belt
//!   Lots are taken by the PackingPlants and the venues before the staffing
//!   (none within 120 ticks of a Farm is left), so its blocks are plots; a
//!   map with neither in range seeds nothing there (phase 3b builds). With
//!   `fill_at_seed` the far posts' holders (longest commute first) wake in
//!   the beds, as the pre-roll houses its movers. The block is a
//!   `BuildingKind::Home` (the view draws it as a Block); `World.
//!   company_homes` names its workplace, and a `Founded` line says so.
//! - **Reserved**: no housing path but [`move_in`] puts anyone in a company
//!   bed ([`open_to_all`] is the filter the other picks read); the staff's
//!   household (a spouse, the children, a kin taken in) lives with them.
//! - **Moving in** ([`move_in`], midnight, before `ownership::rehouse`): the
//!   workplace's staff who are homeless, or were hired since the last
//!   midnight with a Home past `far_ticks`, move in with a co-resident
//!   spouse and children, the nearest block with room for them all (a
//!   spouse on another staff only when the block is no farther from that
//!   work than the Home; else the household stays). The commute-pressure move for the staff already
//!   housed far is phase 4 (I2).
//! - **Rent** (`ownership::collect_rent`): each resident on the staff (and
//!   each in the grace) owes `rent_mult x [rent] base[tier]` a day to the
//!   block's owner; the household's others owe nothing. It is settled from
//!   the wage the moment it is collected (`ownership::pay_rent_from_income`,
//!   with or without `[rent] pay_from_income`): docked, through the one
//!   transfer path (`Flow::Rent`), so no coin is made or lost.
//! - **Eviction** ([`evictions`], midnight): a resident adult with no one in
//!   the household on the staff (a post at the workplace, or at another of
//!   the block's owner's: the corp's bed) is noted; `grace_days` later he is evicted
//!   (`ownership::evict`, the spouse and children with him, an `Evicted`
//!   line naming the job) and goes the homeless and rehouse way.
//!
//! Statistical mirror: none needed beyond the shared code. The hire, the
//! move-in, the rent, the wage it is docked from (`lod`'s Statistical
//! `collect_wage`) and the eviction are tier-blind, so a Statistical worker
//! holds a company bed and pays its docked rent as a Full one does. No
//! random draw. `CITYSIM_COMPANY_HOUSING_OFF=1` drops `[company_housing]`:
//! nothing is seeded, `World.company_homes` stays empty and every filter
//! passes.

use crate::components::{Brain, Building, BuildingKind, Child, Household, Job, Sentence};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

/// `[company_housing]` on.
pub fn on(world: &World) -> bool {
    world.config.company_housing.is_some()
}

/// The workplace whose staff `home`'s beds are reserved for.
pub fn of(world: &World, home: EntityId) -> Option<EntityId> {
    world.company_homes.get(&home).copied()
}

/// `home` is not a company block: open to the city's housing picks.
pub fn open_to_all(world: &World, home: EntityId) -> bool {
    !world.company_homes.contains_key(&home)
}

/// `a` is on the staff company block `home` houses: a post at its
/// workplace, or at another workplace of the block's owner (a worker
/// promoted or moved within the corp keeps the corp's bed; the beds are
/// let to the workplace's staff, `move_in`).
fn on_staff(world: &World, a: EntityId, home: EntityId) -> bool {
    let Some(w) = of(world, home) else { return false };
    let Some(e) = world.comp::<Job>(a).and_then(|j| j.employer) else { return false };
    e == w || world.owner_of(home).is_some_and(|o| world.owner_of(e) == Some(o))
}

/// A resident of company block `home` who owes its rent: on the staff, or
/// in the grace after the job ended.
pub fn payer(world: &World, a: EntityId, home: EntityId) -> bool {
    on_staff(world, a, home) || world.company_lapsed.contains_key(&a)
}

/// `a` lives in a company block (its rent is docked from the wage).
pub fn housed(world: &World, a: EntityId) -> bool {
    world.comp::<Household>(a).and_then(|h| h.home).is_some_and(|h| world.company_homes.contains_key(&h))
}

/// The company rent a day for `home`, in tenths: `rent_mult x [rent]
/// base[tier]` (0 off or for a plain Home).
pub fn rent_tenths(world: &World, home: EntityId) -> i64 {
    let Some(cfg) = world.config.company_housing.as_ref() else { return 0 };
    if !world.company_homes.contains_key(&home) {
        return 0;
    }
    let Some(b) = world.comp::<Building>(home).filter(|b| !b.demolished && !b.derelict) else { return 0 };
    let base = world.config.rent.base[usize::from(b.tier.min(2))];
    ((cfg.rent_mult * base as f32 * 10.0).round() as i64).max(0)
}

fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

fn door(world: &World, b: EntityId) -> Option<crate::components::TilePos> {
    world.comp::<Building>(b).map(|bd| bd.door)
}

/// A post at `w` held by `a` is far: homeless, or housed past `far` of
/// travel from `w`'s door.
fn far_post(world: &World, a: EntityId, wdoor: crate::components::TilePos, far: Tick) -> bool {
    match world.comp::<Household>(a).and_then(|h| h.home).and_then(|h| door(world, h)) {
        None => true,
        Some(hd) => crate::systems::distance::travel(world, hd, wdoor) > far,
    }
}

/// World build, after the staffing and before the housing pre-roll (see
/// the module): the company blocks, and with `fill_at_seed` the far posts'
/// holders moved in. Returns `(blocks, beds)`. No RNG, no coin (a seed
/// building, as the pre-roll's).
pub fn seed(world: &mut World) -> (u32, u32) {
    let Some(cfg) = world.config.company_housing.clone() else { return (0, 0) };
    let mut workplaces: Vec<EntityId> = Vec::new();
    for &k in &cfg.kinds {
        workplaces.extend(world.buildings_of_kind(k).iter().copied().filter(|&b| standing(world, b)));
    }
    workplaces.sort_unstable();
    workplaces.dedup();
    if workplaces.is_empty() {
        return (0, 0);
    }
    // The free beds of the plain Blocks: (door, free).
    let homes: Vec<(crate::components::TilePos, usize)> = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .filter(|&&h| standing(world, h) && open_to_all(world, h))
        .filter_map(|&h| {
            let b = world.comp::<Building>(h)?;
            let free = usize::from(b.capacity).saturating_sub(world.residents_of(h).len());
            (free > 0).then_some((b.door, free))
        })
        .collect();
    let staff = crate::systems::ownership::staff_by_building(world, &workplaces);
    let mut want: Vec<(std::cmp::Reverse<u32>, EntityId)> = Vec::new();
    for &w in &workplaces {
        let Some(wdoor) = door(world, w) else { continue };
        let far = staff.get(&w).map_or(0, |s| s.iter().filter(|&&a| far_post(world, a, wdoor, cfg.far_ticks)).count());
        if far == 0 {
            continue;
        }
        let free: usize = homes
            .iter()
            .filter(|&&(d, _)| crate::systems::distance::travel(world, d, wdoor) <= cfg.far_ticks)
            .map(|&(_, f)| f)
            .sum();
        let n = ((far.saturating_sub(free)) as f32 * cfg.seed_share).ceil() as u32;
        if n >= cfg.min_beds.max(1) {
            want.push((std::cmp::Reverse(n), w));
        }
    }
    want.sort_unstable();
    let pantry = world.config.world.home_pantry_initial;
    let mut occ = Occupancy::new(world);
    let (mut blocks, mut beds) = (0u32, 0u32);
    for (std::cmp::Reverse(mut n), w) in want {
        let Some(wdoor) = door(world, w) else { continue };
        let owner = world.owner_of(w);
        let what = world.name_of(w);
        for _ in 0..cfg.blocks_max {
            if n == 0 {
                break;
            }
            let Some(lot) = next_site(world, &cfg, &mut occ, wdoor, n) else { break };
            if crate::systems::founding::build_on_lot(world, lot, BuildingKind::Home, owner).is_err() {
                break;
            }
            let cap = {
                let Some(b) = world.comp::<Building>(lot) else { continue };
                let interior = usize::from(b.rect.w.saturating_sub(2)) * usize::from(b.rect.h.saturating_sub(2));
                let per = u16::try_from(interior).unwrap_or(u16::MAX).min(cfg.beds_per_floor).max(1);
                crate::systems::founding::with_floors(world, BuildingKind::Home, per, b.floors.max(1))
            };
            if let Some(b) = world.comp_mut::<Building>(lot) {
                b.capacity = cap;
                b.stock_food = pantry;
            }
            world.company_homes.insert(lot, w);
            let rent = rent_tenths(world, lot) / 10;
            if let Some(b) = world.comp_mut::<Building>(lot) {
                b.rent_per_day = rent;
            }
            blocks += 1;
            beds += u32::from(cap);
            n = n.saturating_sub(u32::from(cap));
            let (place, label) = (world.name_of(lot), world.owner_label(owner));
            let mut actors: Vec<EntityId> = owner.into_iter().collect();
            actors.extend([lot, w]);
            world.push_event(
                EventKind::Founded,
                &actors,
                format!("{place} stands as {label}'s company housing for {what}'s staff ({cap} beds)"),
            );
        }
    }
    if cfg.fill_at_seed {
        fill(world, true);
    }
    (blocks, beds)
}

/// The tiles no new block may take: every building's rect (a ring of one
/// round a new block must stay clear of them too) and every tile that is
/// not open ground.
struct Occupancy {
    w: usize,
    h: usize,
    taken: Vec<bool>,
}

impl Occupancy {
    fn new(world: &World) -> Occupancy {
        let (w, h) = (world.map.w(), world.map.h());
        let mut o = Occupancy { w, h, taken: vec![false; w * h] };
        for b in world.with::<Building>() {
            if let Some(r) = world.comp::<Building>(b).map(|bd| bd.rect) {
                o.mark(r, 0);
            }
        }
        o
    }

    fn mark(&mut self, r: crate::components::Rect, ring: usize) {
        let (x0, y0) = (usize::from(r.x).saturating_sub(ring), usize::from(r.y).saturating_sub(ring));
        let x1 = (usize::from(r.x) + usize::from(r.w) + ring).min(self.w);
        let y1 = (usize::from(r.y) + usize::from(r.h) + ring).min(self.h);
        for y in y0..y1 {
            for x in x0..x1 {
                self.taken[y * self.w + x] = true;
            }
        }
    }

    fn taken(&self, x: usize, y: usize) -> bool {
        self.taken[y * self.w + x]
    }
}

/// The next block's site for the workplace at `wdoor`, storeys for the
/// `want` beds up to `floors_max`: the nearest plain vacant Lot within
/// `lot_cap_ticks` (ties lower id), else a `plot_w` x `plot_h` plot of
/// open ground within it (a Lot spawned there): every tile `Ground` and clear of any building
/// with a ring of one, the door mid-side toward the workplace, nearest by
/// travel (ties top row, then left column). `None`: no site in range.
fn next_site(
    world: &mut World,
    cfg: &crate::config::CompanyHousingCfg,
    occ: &mut Occupancy,
    wdoor: crate::components::TilePos,
    want: u32,
) -> Option<EntityId> {
    use crate::components::{Rect, TilePos};
    let lot = crate::systems::founding::vacant_lots(world)
        .into_iter()
        .filter(|&l| !crate::systems::street::is_shanty(world, l))
        .filter_map(|l| {
            let t = crate::systems::distance::travel(world, door(world, l)?, wdoor);
            (t <= cfg.lot_cap_ticks).then_some((t, l))
        })
        .min()
        .map(|(_, l)| l);
    if let Some(l) = lot {
        // Storeys for the beds wanted, the Lot's own at least.
        if let Some(b) = world.comp_mut::<Building>(l) {
            let per = (usize::from(b.rect.w.saturating_sub(2)) * usize::from(b.rect.h.saturating_sub(2)))
                .min(usize::from(cfg.beds_per_floor))
                .max(1) as u32;
            let need = u8::try_from(want.div_ceil(per)).unwrap_or(u8::MAX);
            b.floors = need.clamp(b.floors.max(1), cfg.floors_max.max(b.floors.max(1)));
            let r = b.rect;
            occ.mark(r, 1);
        }
        return Some(l);
    }
    let (pw, ph) = (usize::from(cfg.plot_w.max(3)), usize::from(cfg.plot_h.max(3)));
    let step = world.config.exec.move_ticks_full.max(1);
    let reach = usize::try_from(cfg.lot_cap_ticks / step).unwrap_or(usize::MAX).min(occ.w.max(occ.h));
    let (cx, cy) = (usize::from(wdoor.x), usize::from(wdoor.y));
    let mut best: Option<(Tick, usize, usize, TilePos)> = None;
    let ylo = cy.saturating_sub(reach + ph).max(1);
    let yhi = (cy + reach).min(occ.h.saturating_sub(ph + 1));
    let xlo = cx.saturating_sub(reach + pw).max(1);
    let xhi = (cx + reach).min(occ.w.saturating_sub(pw + 1));
    for y in ylo..=yhi {
        for x in xlo..=xhi {
            // The door: mid-side, on the side facing the workplace.
            let (mx, my) = (x + pw / 2, y + ph / 2);
            let d = if cy >= y + ph {
                (mx, y + ph - 1)
            } else if cy < y {
                (mx, y)
            } else if cx < x {
                (x, my)
            } else {
                (x + pw - 1, my)
            };
            let dp = TilePos { x: d.0 as u16, y: d.1 as u16 };
            let t = crate::systems::distance::travel(world, dp, wdoor);
            if t > cfg.lot_cap_ticks || best.is_some_and(|b| (t, y, x) >= (b.0, b.1, b.2)) {
                continue;
            }
            if plot_clear(world, occ, x, y, pw, ph) {
                best = Some((t, y, x, dp));
            }
        }
    }
    let (_, y, x, dp) = best?;
    let rect = Rect { x: x as u16, y: y as u16, w: pw as u16, h: ph as u16 };
    // A plain Lot's record, cloned and placed (the map's Lots are the
    // template: a map with none has no plot either).
    let template = world.buildings_of_kind(BuildingKind::Lot).first().copied()?;
    let mut b = world.comp::<Building>(template)?.clone();
    b.rect = rect;
    b.door = dp;
    // Storeys for the beds wanted (one at least, `plot_floors` at most).
    let per = ((pw - 2) * (ph - 2)).min(usize::from(cfg.beds_per_floor)).max(1) as u32;
    b.floors = u8::try_from(want.div_ceil(per)).unwrap_or(u8::MAX).clamp(1, cfg.floors_max.max(1));
    b.owner = None;
    b.occupants = Vec::new();
    b.demolished = false;
    b.derelict = false;
    b.empty_since = None;
    b.label = None;
    b.capacity = 0;
    b.stock_food = 0;
    let id = world.spawn();
    world.insert(id, b);
    let list = world.buildings_by_kind.entry(BuildingKind::Lot).or_default();
    if let Err(i) = list.binary_search(&id) {
        list.insert(i, id);
    }
    occ.mark(rect, 1);
    Some(id)
}

/// The plot at `(x, y)`, `pw` x `ph`: every tile open ground, the ring of
/// one round it walkable, none of it any building's.
fn plot_clear(world: &World, occ: &Occupancy, x: usize, y: usize, pw: usize, ph: usize) -> bool {
    use crate::components::TileKind;
    for yy in y - 1..y + ph + 1 {
        for xx in x - 1..x + pw + 1 {
            if occ.taken(xx, yy) {
                return false;
            }
            let inside = yy >= y && yy < y + ph && xx >= x && xx < x + pw;
            let k = world.map.tile(xx, yy);
            if (inside && k != TileKind::Ground) || (!inside && !k.walkable()) {
                return false;
            }
        }
    }
    true
}

/// Midnight, before `ownership::rehouse` (see the module): the staff who
/// are homeless or were hired today far from work move into a company bed.
pub fn move_in(world: &mut World) {
    fill(world, false);
}

/// [`move_in`]; `seed` (world build, `fill_at_seed`): every far post's
/// holder is a candidate, longest commute first, and the movers' bodies
/// wake in the block (the seed city lives where it is housed).
fn fill(world: &mut World, seed: bool) {
    let Some(cfg) = world.config.company_housing.clone() else { return };
    if world.company_homes.is_empty() {
        return;
    }
    let tick = world.tick;
    let since = tick.saturating_sub(TICKS_PER_DAY);
    // The workplace -> its standing blocks, ascending.
    let mut blocks: std::collections::BTreeMap<EntityId, Vec<EntityId>> = std::collections::BTreeMap::new();
    for (&h, &w) in &world.company_homes {
        if standing(world, h) {
            blocks.entry(w).or_default().push(h);
        }
    }
    let workplaces: Vec<EntityId> = blocks.keys().copied().collect();
    let staff = crate::systems::ownership::staff_by_building(world, &workplaces);
    for (w, homes) in blocks {
        let Some(wdoor) = door(world, w) else { continue };
        let Some(staff) = staff.get(&w) else { continue };
        let mut staff = staff.clone();
        if seed {
            // Longest commute first (the homeless first of all), ties lower id.
            let key = |a: EntityId| match world.comp::<Household>(a).and_then(|h| h.home).and_then(|h| door(world, h)) {
                None => Tick::MAX,
                Some(hd) => crate::systems::distance::travel(world, hd, wdoor),
            };
            let mut keyed: Vec<(std::cmp::Reverse<Tick>, EntityId)> =
                staff.iter().map(|&a| (std::cmp::Reverse(key(a)), a)).collect();
            keyed.sort_unstable();
            staff = keyed.into_iter().map(|(_, a)| a).collect();
        }
        for a in staff {
            if !world.has::<Brain>(a)
                || world.has::<Sentence>(a)
                || !crate::systems::law::living(world, a)
                || world.comp::<Brain>(a).is_some_and(|b| b.emigrating)
                || !crate::systems::demography::is_adult(world, a)
            {
                continue;
            }
            let home = world.comp::<Household>(a).and_then(|h| h.home);
            if home.is_some_and(|h| world.company_homes.contains_key(&h)) {
                continue;
            }
            let hired = world.comp::<Job>(a).is_some_and(|j| j.hired_tick >= since && j.hired_tick > 0);
            let take = match home {
                None => true,
                Some(_) => (seed || hired) && far_post(world, a, wdoor, cfg.far_ticks),
            };
            if !take {
                continue;
            }
            // The household: a co-resident spouse and the children sharing
            // the Home. A spouse on another workplace's staff moves only
            // when the block is no farther from that work than the Home
            // (else the household stays, the worker with it).
            let spouse = world.spouse_of(a).filter(|&s| {
                world.has::<Brain>(s)
                    && world.comp::<Household>(s).and_then(|h| h.home) == home
                    && !world.has::<Sentence>(s)
            });
            let spouse_work = spouse
                .and_then(|s| world.comp::<Job>(s).and_then(|j| j.employer))
                .filter(|&e| e != w)
                .and_then(|e| door(world, e));
            let kids: Vec<EntityId> = crate::systems::demography::children_of_agent(world, a)
                .into_iter()
                .filter(|&c| world.has::<Child>(c) && world.comp::<Household>(c).and_then(|h| h.home) == home)
                .collect();
            let need = 1 + usize::from(spouse.is_some()) + kids.len();
            let pick = homes
                .iter()
                .copied()
                .filter(|&h| {
                    world
                        .comp::<Building>(h)
                        .is_some_and(|b| world.residents_of(h).len() + need <= usize::from(b.capacity))
                })
                .filter_map(|h| Some((crate::systems::distance::travel(world, door(world, h)?, wdoor), h)))
                .min()
                .map(|(_, h)| h);
            let Some(to) = pick else { continue };
            if let (Some(sw), Some(to_door)) = (spouse_work, door(world, to)) {
                let now = home.and_then(|h| door(world, h)).map(|hd| crate::systems::distance::travel(world, hd, sw));
                if now.is_some_and(|t| crate::systems::distance::travel(world, to_door, sw) > t) {
                    continue;
                }
            }
            let movers: Vec<EntityId> = std::iter::once(a).chain(spouse).collect();
            for &m in movers.iter().chain(kids.iter()) {
                world.set_home(m, Some(to));
                if let Some(h) = world.comp_mut::<Household>(m) {
                    h.rent_due = h.rent_due.min(0.0);
                    h.arrears = 0;
                }
                if seed {
                    wake_in(world, m, to);
                }
            }
            if seed {
                // A seed household: no event (it lived there before day 0).
                continue;
            }
            let (name, place, what) = (world.name_of(a), world.name_of(to), world.name_of(w));
            let label = world.owner_label(world.owner_of(to));
            world.push_event(
                EventKind::Housed,
                &[a, to],
                format!("{name} moved into {place}, {label}'s company housing for {what}'s staff"),
            );
            world.stats.current.housed += 1;
            crate::systems::ownership::jobs2_row(world).company_moved_in += 1;
        }
    }
}

/// World build: `m`'s body, inside the Block it was dealt into, wakes in
/// `to` (as the pre-roll moves its movers).
fn wake_in(world: &mut World, m: EntityId, to: EntityId) {
    let Some(at) = world.comp::<crate::components::Position>(m).and_then(|p| p.building) else { return };
    if at == to || world.comp::<Building>(at).is_none_or(|b| b.kind != BuildingKind::Home) {
        return;
    }
    let Some(d) = door(world, to) else { return };
    let tick = world.tick;
    world.remove_from_building(m);
    if let Some(p) = world.comp_mut::<crate::components::Position>(m) {
        p.tile = d;
        p.building = None;
        p.entered = tick;
    }
    world.enter_building(m, to);
}

/// Midnight, before [`move_in`] (see the module): the blocks whose
/// workplace is gone become plain Homes; a resident household with no one
/// on the staff is noted, and evicted `grace_days` later.
pub fn evictions(world: &mut World) {
    let Some(cfg) = world.config.company_housing.clone() else { return };
    if world.company_homes.is_empty() {
        return;
    }
    let gone: Vec<EntityId> = world
        .company_homes
        .iter()
        .filter(|&(&h, &w)| !standing(world, h) || !standing(world, w))
        .map(|(&h, _)| h)
        .collect();
    for h in gone {
        world.company_homes.remove(&h);
    }
    let tick = world.tick;
    let grace = u64::from(cfg.grace_days) * TICKS_PER_DAY;
    let mut entitled: std::collections::BTreeSet<EntityId> = std::collections::BTreeSet::new();
    let mut due: Vec<(EntityId, EntityId, EntityId)> = Vec::new();
    let mut lapsed_now: Vec<EntityId> = Vec::new();
    let (mut beds, mut held) = (0u32, 0u32);
    for (&h, &w) in &world.company_homes {
        beds += world.comp::<Building>(h).map_or(0, |b| u32::from(b.capacity));
        let residents = world.residents_of(h);
        held += residents.len() as u32;
        let staffed: Vec<EntityId> = residents.iter().copied().filter(|&r| on_staff(world, r, h)).collect();
        for &r in residents {
            if world.has::<Child>(r) || !world.has::<Brain>(r) {
                continue;
            }
            // On the staff, or the household of one (a spouse, a parent
            // or a child of the staff member under this roof).
            let kin = staffed.iter().any(|&s| {
                s == r
                    || world.spouse_of(s) == Some(r)
                    || crate::systems::demography::is_child_of(world, r, s)
                    || crate::systems::demography::is_child_of(world, s, r)
            });
            if kin {
                entitled.insert(r);
                continue;
            }
            match world.company_lapsed.get(&r) {
                None => lapsed_now.push(r),
                Some(&t) if tick >= t + grace => due.push((r, h, w)),
                Some(_) => {}
            }
        }
    }
    {
        let row = crate::systems::ownership::jobs2_row(world);
        row.company_beds = beds;
        row.company_housed = held;
    }
    // A noted resident back on the staff (or housed elsewhere) is no longer
    // lapsed.
    let stale: Vec<EntityId> =
        world.company_lapsed.keys().copied().filter(|&a| entitled.contains(&a) || !housed(world, a)).collect();
    for a in stale {
        world.company_lapsed.remove(&a);
    }
    for a in lapsed_now {
        world.company_lapsed.insert(a, tick);
    }
    for (a, h, w) in due {
        world.company_lapsed.remove(&a);
        if world.comp::<Household>(a).and_then(|x| x.home) != Some(h) {
            continue;
        }
        let why = format!("the job at {} ended: company housing", world.name_of(w));
        let before = world.stats.current.evictions;
        crate::systems::ownership::evict(world, a, &why);
        let n = world.stats.current.evictions.saturating_sub(before);
        crate::systems::ownership::jobs2_row(world).company_evicted += n as u32;
        // The spouse put out with him is no longer lapsed.
        if let Some(s) = world.spouse_of(a) {
            world.company_lapsed.remove(&s);
        }
    }
}
