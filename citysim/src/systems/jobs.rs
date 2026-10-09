//! Life pass L2 § 1 (plan phase 1): the economy of jobs. The seven new
//! kinds seeded on Lots (`seed_venues`), the staffing overrides (`full_staff`,
//! `top_up`), the Fab's work (`accrue_fab_work`), the scrap chain, the
//! NoodleBars' restock, the venues' daily prices and the corps' import
//! tally. Nothing here runs per agent per tick; every entry point is a
//! no-op unless [`on`].

use crate::components::{Building, BuildingKind, Corp, CorpOrder, Good, Job, Niche, Role, Skills, TilePos};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time;
use crate::world::World;

/// Units a NoodleBar orders from the Market (seeding and the restock, L38).
const NOODLE_ORDER: u32 = 30;

/// Plan L8: Sanitation sweeps the beat (`Sweep`), not `TendGraves`.
pub fn sweep_on(world: &World) -> bool {
    world.config.jobs.sweep
}

/// Plan L3: a building's full staff: `[jobs] market_staff` / `bar_staff`
/// with jobs on, else `[buildings] <kind>.staff`.
pub fn full_staff(world: &World, kind: BuildingKind) -> usize {
    match kind {
        BuildingKind::Market => return world.config.jobs.market_staff as usize,
        BuildingKind::Bar => return world.config.jobs.bar_staff as usize,
        _ => {}
    }
    world.config.buildings.for_kind(kind).staff as usize
}

/// A standing building (not demolished, not derelict).
fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

/// The standing building of `kind` whose door is nearest `from` (ties lower id).
pub fn nearest_of_kind(world: &World, kind: BuildingKind, from: TilePos) -> Option<EntityId> {
    world
        .buildings_of_kind(kind)
        .iter()
        .copied()
        .filter(|&b| standing(world, b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .min()
        .map(|(_, b)| b)
}

/// Spec § 1 "Prices by tier": `round(price_base[kind][tier] × level)`;
/// `level` a Food corp's Food `price_level` for its NoodleBar, else 1.0;
/// a gang front × `front_markup`. A NoodleBar's base is the nearest
/// Market's food price plus `noodle_bar_markup`. 0 = not sold at this tier.
pub fn venue_price(world: &World, b: EntityId, kind: BuildingKind, tier: u8, owner: Option<EntityId>) -> i64 {
    let cfg = &world.config.leisure;
    let mut base = cfg.price_base.of(kind, tier);
    if kind == BuildingKind::NoodleBar {
        let door = world.comp::<Building>(b).map(|bd| bd.door).unwrap_or_default();
        let market = nearest_of_kind(world, BuildingKind::Market, door)
            .and_then(|m| world.comp::<crate::components::Market>(m))
            .map_or(world.config.world.price_initial, |m| m.price_food);
        base += market;
    }
    if base <= 0 {
        return 0;
    }
    let mut level = 1.0f32;
    if let Some(c) = owner.and_then(|o| world.comp::<Corp>(o)) {
        if kind == BuildingKind::NoodleBar && c.niches.contains(&Niche::Food) {
            level = c.price_level.get(&Niche::Food).copied().unwrap_or(1.0);
        }
    }
    let front = world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).and_then(|v| v.front_of).is_some()
        || owner.is_some_and(|o| world.has::<crate::components::Gang>(o));
    if front {
        level *= cfg.front_markup;
    }
    ((base as f32 * level).round() as i64).max(1)
}

// ---------------------------------------------------------------------------
// Seeding (plan L7, the Seeding section)
// ---------------------------------------------------------------------------

/// The seeding order: scarce Lot sizes first.
const SEED_ORDER: [BuildingKind; 7] = [
    BuildingKind::Fab,
    BuildingKind::Lounge,
    BuildingKind::Club,
    BuildingKind::FightPit,
    BuildingKind::Den,
    BuildingKind::Arcade,
    BuildingKind::NoodleBar,
];

/// The seeded corps by row (`Corp.slot`), ascending.
fn corps_by_row(world: &World) -> Vec<(u8, EntityId)> {
    let mut v: Vec<(u8, EntityId)> = world
        .corps()
        .into_iter()
        .filter_map(|c| world.comp::<Corp>(c).and_then(|cc| cc.slot).map(|s| (s, c)))
        .collect();
    v.sort_unstable();
    v
}

fn corp_named(world: &World, name: &str) -> Option<EntityId> {
    world.corps().into_iter().find(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.name == name))
}

/// The owner of copy `i` of `kind` (the Seeding table; the fronts below):
/// `(owner, front_of)`;
/// `None` for the Fab = skip the copy (no Tech-capable row left).
fn seed_owner(world: &World, kind: BuildingKind, i: usize) -> Option<(Option<EntityId>, Option<EntityId>)> {
    let rows = corps_by_row(world);
    let in_niche = |n: Niche| -> Vec<EntityId> {
        rows.iter()
            .filter(|&&(_, c)| world.comp::<Corp>(c).is_some_and(|cc| cc.niches.contains(&n)))
            .map(|&(_, c)| c)
            .collect()
    };
    let gangs: Vec<EntityId> =
        world.gang_list().iter().copied().filter(|&g| !crate::systems::creeds::is_purist(world, g)).collect();
    Some(match kind {
        BuildingKind::Fab => {
            let name = ["Zetatech", "Militech"].get(i)?;
            (Some(corp_named(world, name)?), None)
        }
        // Fix round (phase 1 deviation, as the fronts below; Seeding table:
        // "the megacorp rows"): the seeded Lounge and Clubs stand on the
        // city's deed until phase 2 gives them takings. Megacorp-owned, the
        // 26 Hosts and Concierges (~170 coins a day of wages and 40 of
        // upkeep, no revenue) drained Arasaka from 3.7k to 0-1k by day 60-90
        // on seeds 42-44 (main: 7-14k), and Arasaka's Squeeze in Security is
        // what carries M11's "Squeeze held" bullet.
        BuildingKind::Lounge | BuildingKind::Club => (None, None),
        // Phase 1 deviation (Seeding table: "owner the gang"): the seeded
        // fronts stand on the city's deed with `front_of` the gang. Gangs
        // hold 50 coins at seed and a front earns nothing before phase 2's
        // bets, so gang-owned fronts left every Croupier and Fighter unpaid
        // from day 1 and drained both gangs to 0. Phase 2 hands the deed over.
        BuildingKind::FightPit | BuildingKind::Den => match gangs.get(i) {
            Some(&g) => (None, Some(g)),
            None => (None, None),
        },
        BuildingKind::Arcade => {
            let tech = in_niche(Niche::Tech);
            (if tech.is_empty() { None } else { Some(tech[i % tech.len()]) }, None)
        }
        BuildingKind::NoodleBar => {
            // Every third the city; the rest round-robin by row, Nutrix first.
            if i % 3 == 2 {
                (None, None)
            } else {
                let mut food = in_niche(Niche::Food);
                if let Some(n) = corp_named(world, "Nutrix") {
                    if let Some(p) = food.iter().position(|&c| c == n) {
                        food.remove(p);
                        food.insert(0, n);
                    }
                }
                let k = i - i / 3;
                (if food.is_empty() { None } else { Some(food[k % food.len()]) }, None)
            }
        }
        _ => return None,
    })
}

/// L2 phase 2 (the Seeding table's owners): the Lounge's heir is the corp
/// row with the largest `treasury_initial`, copy `i` of the Clubs the corps
/// round-robin by row; `None` for the other kinds or without corps.
fn seed_heir(world: &World, kind: BuildingKind, i: usize) -> Option<EntityId> {
    let rows = corps_by_row(world);
    if rows.is_empty() {
        return None;
    }
    match kind {
        BuildingKind::Lounge => rows
            .iter()
            .map(|&(s, c)| {
                (world.config.corps.treasury_initial.get(usize::from(s)).copied().unwrap_or(0), std::cmp::Reverse(s), c)
            })
            .max()
            .map(|(_, _, c)| c),
        BuildingKind::Club => Some(rows[i % rows.len()].1),
        _ => None,
    }
}

/// The vacant Lot for the next copy of `kind`: an allowed door tier, an
/// interior of at least `min(capacity, 6)` tiles; least `(copies of the kind
/// in its district, door distance to the district's centroid, id)`.
fn seed_lot(world: &World, kind: BuildingKind) -> Option<EntityId> {
    let need = usize::from(world.config.buildings.for_kind(kind).capacity.min(6));
    crate::systems::founding::vacant_lots(world)
        .into_iter()
        .filter_map(|l| {
            let bd = world.comp::<Building>(l)?;
            let tier = crate::systems::founding::door_tier(world, bd.door);
            let interior = usize::from(bd.rect.w.saturating_sub(2)) * usize::from(bd.rect.h.saturating_sub(2));
            if !crate::systems::founding::tier_ok(kind, tier) || interior < need {
                return None;
            }
            let d = world.district_of_building(l);
            let copies = world
                .buildings_of_kind(kind)
                .iter()
                .filter(|&&b| standing(world, b) && world.district_of_building(b) == d)
                .count();
            let centroid = world.districts.get(d.index()).map_or(bd.door, |x| x.centroid);
            Some((copies, bd.door.manhattan(centroid), l))
        })
        .min()
        .map(|(_, _, l)| l)
}

/// Plan L7: the venues and Fabs of the Seeding section on the vacant Lots
/// (no RNG), each with its full staff posted, its `Venue` and (a
/// NoodleBar) 30 food from the nearest Market. Returns the buildings in
/// seeding order. A no-op unless [`on`].
pub fn seed_venues(world: &mut World) -> Vec<EntityId> {
    let mut built = Vec::new();
    for kind in SEED_ORDER {
        let n = world.config.jobs.seed_venues.count(kind) as usize;
        for i in 0..n {
            let Some((owner, front)) = seed_owner(world, kind, i) else { continue };
            let Some(lot) = seed_lot(world, kind) else { continue };
            if crate::systems::founding::build_on_lot(world, lot, kind, owner).is_err() {
                continue;
            }
            // L2 phase 2: the owner a city-deeded Club or Lounge passes to
            // once it earns (`leisure::hand_over`; a front's is its gang).
            let heir = seed_heir(world, kind, i);
            if let Some(v) = world.comp_mut::<Building>(lot).and_then(|bd| bd.venue.as_mut()) {
                v.front_of = front;
                v.heir = heir;
            }
            if front.is_some() {
                let (tier, o) = world.comp::<Building>(lot).map_or((0, None), |bd| (bd.tier, bd.owner));
                let price = venue_price(world, lot, kind, tier, o);
                if let Some(v) = world.comp_mut::<Building>(lot).and_then(|bd| bd.venue.as_mut()) {
                    v.price = price;
                }
            }
            if kind == BuildingKind::NoodleBar {
                restock_noodle_bar(world, lot);
            }
            let at = world.districts.get(world.district_of_building(lot).index()).map_or("?", |x| x.name.as_str());
            let who = world.owner_label(owner);
            let text = format!("{who} opened a {} in {at} (seeded; Lot {})", kind.label(), lot.index);
            let mut actors: Vec<EntityId> = owner.into_iter().collect();
            actors.push(lot);
            world.push_event(EventKind::Founded, &actors, text);
            built.push(lot);
        }
    }
    world.venues_seeded = !built.is_empty();
    built
}

// ---------------------------------------------------------------------------
// Staffing (plan L3)
// ---------------------------------------------------------------------------

/// Plan L3, daily: every standing Market and Bar (any owner, the city's
/// too) below [`full_staff`] posts its deficit; a hunkering corp's staffs
/// to half (its order's floor). Then the venues' and Fabs' recovery.
pub fn top_up(world: &mut World) {
    // Real economy phase 2 (plan E18): with wages on a corp's buildings are
    // staffed by the margin rule (`wages::staff`); the top-up keeps the
    // city's and agents' buildings only.
    let wages = crate::systems::wages::on(world);
    let corp_owned = |world: &World, b: EntityId| wages && world.corp_of_building(b).is_some();
    for kind in [BuildingKind::Market, BuildingKind::Bar] {
        let Some(role) = ownership::role_for(kind) else { continue };
        let full = full_staff(world, kind);
        for b in world.buildings_of_kind(kind).to_vec() {
            if !standing(world, b) || corp_owned(world, b) {
                continue;
            }
            // Deviation (Seeding section: "Markets and Bars get their top-up
            // vacancies the first midnight"): the whole deficit is posted,
            // not one a day.
            let short = deficit(world, b, role, full);
            if short > 0 {
                world.vacancies.entry(b).or_default().extend(std::iter::repeat_n(role, short));
            }
        }
    }
    // Review fix: every standing leisure venue and Fab below its staff (an
    // unpaid quit posts no vacancy) re-posts one vacancy a day once its
    // owner's purse covers a day's wage (a hunkering corp's to half).
    let mut l2 = BuildingKind::LEISURE.to_vec();
    l2.push(BuildingKind::Fab);
    for kind in l2 {
        let Some(role) = ownership::role_for(kind) else { continue };
        let full = full_staff(world, kind);
        let wage = world.config.economy.wage(role);
        for b in world.buildings_of_kind(kind).to_vec() {
            if !standing(world, b) || corp_owned(world, b) {
                continue;
            }
            if deficit(world, b, role, full) > 0 && world.purse(world.owner_of(b)) >= wage {
                world.vacancies.entry(b).or_default().push(role);
            }
        }
    }
}

/// `b`'s unfilled `role` places below `full` (a hunkering corp's building
/// staffs to half, Hunker's own floor, so the top-up and Hunker's layoffs
/// do not churn a hire a day): `full` less the staff in `role` and the
/// vacancies already posted for it.
fn deficit(world: &World, b: EntityId, role: Role, full: usize) -> usize {
    let hunker = world.owner_of(b).and_then(|o| world.comp::<Corp>(o)).is_some_and(|c| c.order == CorpOrder::Hunker);
    let full = if hunker { full.div_ceil(2) } else { full };
    let working = ownership::staff_at(world, b)
        .into_iter()
        .filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role == role))
        .count();
    let open = world.vacancies.get(&b).map_or(0, |v| v.iter().filter(|&&r| r == role).count());
    full.saturating_sub(working + open)
}

// ---------------------------------------------------------------------------
// The Fab and the scrap chain (plan L9)
// ---------------------------------------------------------------------------

/// Plan L9: `accrue_farm_work`'s shape: `fab_yield × (fab_skill_floor +
/// fab_skill_slope × farming) × comp_mult` Parts per worker-hour into
/// `production_accum`; whole Parts into the Fab's stock (capped).
pub fn accrue_fab_work(world: &mut World, worker: EntityId, fab: EntityId, ticks: u64) {
    let cfg = &world.config.jobs;
    let farming = world.comp::<Skills>(worker).map_or(0.0, |s| s.farming);
    let mut per_hour = cfg.fab_yield * (cfg.fab_skill_floor + cfg.fab_skill_slope * farming);
    if let Some(c) = world.corp_of_building(fab) {
        per_hour *= crate::systems::competence::comp_mult(world, c);
    }
    let hours = ticks as f32 / time::TICKS_PER_HOUR as f32;
    let whole = {
        let Some(b) = world.comp_mut::<Building>(fab) else { return };
        b.production_accum += per_hour * hours;
        let whole = b.production_accum.floor();
        b.production_accum -= whole;
        whole as u32
    };
    let added = world.add_stock(fab, Good::Parts, whole);
    world.stats.current.living.fab_parts += added;
    // Real economy phase 2 (plan E13): inputs to the World per Part made.
    let per = world.config.economy2.input_per_part;
    crate::systems::wages::produce_inputs(world, fab, added, per);
}

/// Plan L9: a scavenger's find adds a scrap with jobs on.
pub fn add_scrap(world: &mut World) {
    world.scrap = world.scrap.saturating_add(1);
}

/// Plan L9: the scavenge find's chance: `scavenge_p × max(1, 0.5 +
/// litter(d))` with jobs on (`d` the agent's district, litter its dirty
/// share 0..1). L2 phase 5: floored at `scavenge_p` (the spec's `0.5 +
/// litter` halved the L1 find rate where streets are clean; measured litter
/// after day 30 sits at 0.00-0.22 by district with L2 on and 0.00-0.41
/// with it off, so the poorest, released prisoners first, lost half their
/// last income: 10 of seed 42's 26 starvation deaths had left the Precinct
/// within 20 days). Dirty streets still pay more.
pub fn scavenge_p(world: &World, id: EntityId) -> f32 {
    let p = world.config.life.scavenge_p;
    let litter = world
        .comp::<crate::components::Position>(id)
        .map(|pos| world.district_of(pos.tile))
        .and_then(|d| world.districts.get(d.index()))
        .map_or(0.0, |d| d.litter.clamp(0.0, 1.0));
    p * (0.5 + litter).max(1.0)
}

/// Plan L9: a seller owner's `Flow::Import` coins go into its 14-day tally
/// (a corp's only: the Fab trigger).
pub fn note_import(world: &mut World, owner: Option<EntityId>, coins: i64) {
    if coins <= 0 {
        return;
    }
    world.stats.current.living.parts_imported += coins;
    let Some(c) = owner.filter(|&o| world.has::<Corp>(o)) else { return };
    let v = world.jobs_book.corp_imports.entry(c).or_default();
    if v.is_empty() {
        v.push_back(0);
    }
    if let Some(today) = v.back_mut() {
        *today += coins;
    }
}

/// Plan L9: does this Tech corp want a Fab (none owned, 14-day imports over
/// the trigger)?
pub fn wants_fab(world: &World, corp: EntityId) -> bool {
    ownership::owned_of_kind(world, Some(corp), BuildingKind::Fab).is_empty()
        && world.jobs_book.imports_of(corp) > world.config.jobs.fab_import_trigger
}

/// Plan L38: a NoodleBar below `NOODLE_ORDER` buys up to that much from the
/// nearest Market at `wholesale` (the owner pays the Market's owner).
fn restock_noodle_bar(world: &mut World, nb: EntityId) {
    let Some((door, have, owner)) = world.comp::<Building>(nb).map(|bd| (bd.door, bd.stock_food, bd.owner)) else {
        return;
    };
    if have >= NOODLE_ORDER {
        return;
    }
    let Some(m) = nearest_of_kind(world, BuildingKind::Market, door) else { return };
    let stock = world.comp::<Building>(m).map_or(0, |bd| bd.stock_food);
    let room = world.goods_cap(nb, Good::Food).saturating_sub(have);
    let units = NOODLE_ORDER.min(stock).min(room);
    if units == 0 {
        return;
    }
    let price = world.config.corps.wholesale.max(0) * i64::from(units);
    let seller = world.owner_of(m);
    if !matches!(ownership::owner_kind(world, owner), ownership::OwnerKind::City | ownership::OwnerKind::Corp(_))
        && world.purse(owner) < price
    {
        return;
    }
    ownership::charge(world, owner, seller, price, Flow::Wholesale);
    world.take_stock(m, Good::Food, units);
    world.add_stock(nb, Good::Food, units);
}

/// The Recycler (the city's `Cemetery`).
fn recycler(world: &World) -> Option<EntityId> {
    world.building_of_kind(BuildingKind::Cemetery)
}

/// Plan L4, L9, L38, midnight: scrap into Recycler Parts, the NoodleBars'
/// restock, the venues' prices and visit windows, the import windows.
pub fn daily(world: &mut World) {
    // Scrap -> Parts at the Recycler, the remainder kept.
    let per = world.config.jobs.scrap_per_part.max(1);
    if let Some(r) = recycler(world) {
        let parts = world.scrap / per;
        if parts > 0 {
            let added = world.add_stock(r, Good::Parts, parts);
            world.scrap -= added * per;
            world.stats.current.living.scrap_parts += added;
        }
    }
    for nb in world.buildings_of_kind(BuildingKind::NoodleBar).to_vec() {
        if standing(world, nb) {
            restock_noodle_bar(world, nb);
        }
    }
    price_venues(world);
    // The import windows: today closes, a new day opens; empty tallies go.
    for v in world.jobs_book.corp_imports.values_mut() {
        v.push_back(0);
        while v.len() > crate::living::IMPORT_DAYS {
            v.pop_front();
        }
    }
    let gone: Vec<EntityId> = world
        .jobs_book
        .corp_imports
        .iter()
        .filter(|(&c, v)| !world.has::<Corp>(c) || v.iter().all(|&x| x == 0))
        .map(|(&c, _)| c)
        .collect();
    for c in gone {
        world.jobs_book.corp_imports.remove(&c);
    }
}

/// The venues' daily price and visit windows (spec § 1, plan L4).
fn price_venues(world: &mut World) {
    // Real economy phase 2 (plan E20): prices follow the payroll-weighted
    // mean `wage_rev` city-wide at `venue_wage_pass` (1.0 with wages off).
    let pass = crate::systems::wages::venue_price_mult(world);
    for kind in BuildingKind::LEISURE {
        for b in world.buildings_of_kind(kind).to_vec() {
            let Some((tier, owner)) =
                world.comp::<Building>(b).filter(|bd| bd.venue.is_some()).map(|bd| (bd.tier, bd.owner))
            else {
                continue;
            };
            let mut price = venue_price(world, b, kind, tier, owner);
            if pass != 1.0 && price > 0 {
                price = ((price as f32 * pass).round() as i64).max(1);
            }
            if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
                v.price = price;
                let today = std::mem::take(&mut v.visits_today);
                v.visits.push_back(today);
                while v.visits.len() > crate::living::VISIT_DAYS {
                    v.visits.pop_front();
                }
                v.take_today = 0;
            }
        }
    }
}

/// A venue had a visit (a `Drink` at a Club or Den, phase 2's rungs).
pub fn note_visit(world: &mut World, b: EntityId) {
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return };
    let Some(i) = BuildingKind::LEISURE.iter().position(|&k| k == kind) else { return };
    if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
        v.visits_today = v.visits_today.saturating_add(1);
    }
    world.stats.current.living.visits[i] += 1;
}

// ---------------------------------------------------------------------------
// Sweep (plan L8)
// ---------------------------------------------------------------------------

/// The district a sweeper works: its midnight beat, else the Recycler's.
pub fn beat_of(world: &World, id: EntityId) -> Option<crate::components::DistrictId> {
    world.sweep_beats.get(&id).copied().or_else(|| recycler(world).map(|r| world.district_of_building(r)))
}

/// Is `id` a sweeper with Sweep on?
pub fn is_sweeper(world: &World, id: EntityId) -> bool {
    sweep_on(world) && world.comp::<Job>(id).is_some_and(|j| j.role == Role::Sanitation)
}

/// L2 L8: a sweeper on a street tile of its beat district.
pub fn on_beat(world: &World, id: EntityId) -> bool {
    if !is_sweeper(world, id) {
        return false;
    }
    let Some(p) = world.comp::<crate::components::Position>(id).filter(|p| p.building.is_none()) else {
        return false;
    };
    beat_of(world, id) == Some(world.district_of(p.tile))
}

/// L2 L8 (`LocationKey::Beat`): the dirtiest street tile of the sweeper's beat.
pub fn beat_tile(world: &World, id: EntityId) -> Option<TilePos> {
    crate::systems::litter::dirtiest_tile(world, beat_of(world, id)?)
}

/// An L2 kind (a leisure venue or a Fab): its staff work inside it.
pub fn is_l2_building(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| bd.kind.is_leisure() || bd.kind == BuildingKind::Fab)
}

/// Plan L8: an hour-proportional sweep of the beat's dirtiest street tiles
/// (`sweep_per_hour × hours`), and the D23 credit skip for today.
pub fn sweep_done(world: &mut World, id: EntityId, ticks: u64) {
    let Some(d) = beat_of(world, id) else { return };
    let hours = ticks as f32 / time::TICKS_PER_HOUR as f32;
    let units = (world.config.jobs.sweep_per_hour as f32 * hours).round() as u32;
    crate::systems::litter::clean_dirtiest(world, d, units);
    if !world.jobs_book.swept.contains(&id) {
        world.jobs_book.swept.push(id);
    }
}

// ---------------------------------------------------------------------------
// God commands (plan L37)
// ---------------------------------------------------------------------------

/// `OpenVenue`: a `kind` on the vacant Lot nearest district `d`'s centroid
/// (the district's own first), else a derelict Block there refitted, free.
/// A gang owner makes a leisure venue its front.
pub fn open_venue(
    world: &mut World,
    kind: BuildingKind,
    d: crate::components::DistrictId,
    owner: Option<EntityId>,
) -> Result<EntityId, String> {
    if !(kind.is_leisure() || kind == BuildingKind::Fab) {
        return Err(format!("{} is no L2 kind", kind.label()));
    }
    let centroid = world.districts.get(d.index()).map(|x| x.centroid).ok_or("no such district")?;
    let lot = crate::systems::founding::vacant_lots(world)
        .into_iter()
        .filter_map(|l| {
            world.comp::<Building>(l).map(|b| (world.district_of_building(l) != d, b.door.manhattan(centroid), l))
        })
        .min()
        .map(|(_, _, l)| l);
    let b = match lot {
        Some(l) => crate::systems::founding::build_on_lot(world, l, kind, owner)?,
        None => {
            let derelict = crate::systems::street::derelicts(world)
                .into_iter()
                .filter_map(|x| {
                    let bd = world.comp::<Building>(x).filter(|bd| bd.kind == BuildingKind::Home)?;
                    Some((world.district_of_building(x) != d, bd.door.manhattan(centroid), x))
                })
                .min()
                .map(|(_, _, x)| x)
                .ok_or("no vacant Lot or derelict")?;
            crate::systems::founding::refit_with(world, derelict, kind, owner, false)?
        }
    };
    if owner.is_some_and(|o| world.has::<crate::components::Gang>(o)) {
        if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
            v.front_of = owner;
        }
    }
    if kind == BuildingKind::NoodleBar {
        restock_noodle_bar(world, b);
    }
    crate::systems::virt::relink(world);
    Ok(b)
}

/// `HireAll`: every open vacancy at a building of `kind` filled now (the
/// candidate `demography::job_search` would pick). Returns the hires.
pub fn hire_all(world: &mut World, kind: BuildingKind) -> u32 {
    let mut n = 0;
    for b in world.buildings_of_kind(kind).to_vec() {
        let roles = world.vacancies.get(&b).cloned().unwrap_or_default();
        for role in roles {
            let Some(id) = crate::systems::demography::hire_candidate(world, b, role) else { continue };
            crate::systems::demography::hire(world, id, b, role);
            if let Some(v) = world.vacancies.get_mut(&b) {
                if let Some(i) = v.iter().position(|&r| r == role) {
                    v.remove(i);
                }
                if v.is_empty() {
                    world.vacancies.remove(&b);
                }
            }
            n += 1;
        }
    }
    n
}
