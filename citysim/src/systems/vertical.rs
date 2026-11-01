//! The city milestone C8 (docs/CITY_MILESTONE.md § C8): the vertical gangs.
//!
//! - **A Hideout per district.** A gang's seat (`Gang.hideout`, the map's) keeps the stash, the armoury, the
//!   node and the chest; at the build it moves up to the top storey of its host ([`seed`], `seat_up`: the flats
//!   under it keep the rest of the span). Each midnight ([`daily`]) a gang founds a Hideout in each district where
//!   it holds a turf cell and has none (`Gang.district_hideouts`): a vacant unit of a host there, on its top
//!   storey, over a front when one stands (a venue or a shop on the ground), the key money to the landlord
//!   (`Flow::Sale`; a poorer gang moves in on what it has: nobody evicts a gang). A member's Hideout is the
//!   nearest of his gang's to his Home door ([`own_hideout`]): `LocationKey::Hideout`, the bed, the watch, the
//!   lie-low; the raid's muster and its departure take the gang's nearest to the target ([`nearest_to`]).
//! - **The lift is turf** ([`lift_holds`]): a gang holds the lift of a host its Hideout stands in while it holds
//!   the cell of the host's door (`LiftState.held_by`): its members ride first, anyone else waits
//!   `held_wait_ticks` more in the lobby or takes the stairs (`levels::plan`). A raid won or a Hideout sacked
//!   cuts the loser's lift ([`raid_cut`]); a landlord short on a host's power cuts its lift ([`power_cut`]); a cut
//!   ends at `cut_until`. Each change is a line naming who and why.
//! - **The raid climbs** ([`climb_column`]): a raid whose target stands above the ground climbs to it from the
//!   door before the fight, by the lift with `lift_max_raiders` or fewer and the lift running and not held
//!   against it, else by the stairs (the lobby is the choke); the bodies inside on the floors it passes hear it
//!   (`hear_cap`, the noise is word: a line and a sighting); it fights at the target's floor; the guards
//!   answering a corp raid climb too (`raid::corp_brawl`: those who cannot make the floor in the answer's
//!   reach stay out). Under 18 nobody is a target on any floor: the defenders and the column are the raid's
//!   (`ages::is_protected` out), and hearing harms nobody.
//! - **Statistical mirror:** the order-rates ledger files a homeless member under his own Hideout's district
//!   (`fviolence::member_district`), as it filed him under the one Hideout's; a Statistical body does not climb.

use serde::{Deserialize, Serialize};

use crate::components::{Building, BuildingKind, Gang, Position};
use crate::config::VerticalCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::levels;
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_DAY};
use crate::world::World;

/// A raid's column on its climb to the target's floor (`Gang.raid_climb`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RaidClimb {
    pub target: EntityId,
    pub core: EntityId,
    pub floor: u8,
    pub start: Tick,
    pub board: Tick,
    pub arrive: Tick,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lift: bool,
    pub column: Vec<EntityId>,
}

pub fn cfg(world: &World) -> Option<&VerticalCfg> {
    world.config.vertical.as_ref()
}

pub fn on(world: &World) -> bool {
    world.config.vertical.is_some()
}

fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished)
}

/// Every standing Hideout of `gang`, the seat first.
pub fn hideouts_of(world: &World, gang: EntityId) -> Vec<EntityId> {
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    std::iter::once(g.hideout).chain(g.district_hideouts.iter().copied()).filter(|&b| standing(world, b)).collect()
}

/// `b` is one of `gang`'s Hideouts (the seat or a district's).
pub fn is_hideout_of(world: &World, gang: EntityId, b: EntityId) -> bool {
    world.comp::<Gang>(gang).is_some_and(|g| g.hideout == b || g.district_hideouts.contains(&b))
}

/// `gang`'s Hideout nearest `t` by the door (Manhattan; ties the seat, then the lower id). The seat alone off.
pub fn nearest_to(world: &World, gang: EntityId, t: TilePos) -> Option<EntityId> {
    let g = world.comp::<Gang>(gang)?;
    if g.district_hideouts.is_empty() || !on(world) {
        return world.hideout_of(gang);
    }
    std::iter::once(g.hideout)
        .chain(g.district_hideouts.iter().copied())
        .enumerate()
        .filter_map(|(i, b)| {
            let bd = world.comp::<Building>(b).filter(|bd| !bd.demolished)?;
            Some((bd.door.manhattan(t), usize::from(i != 0), b))
        })
        .min()
        .map(|x| x.2)
}

/// A member's own Hideout: his gang's nearest to his Home door (or where he stands, homeless). The seat off.
pub fn own_hideout(world: &World, id: EntityId) -> Option<EntityId> {
    let gang = world.gang_of(id)?;
    let g = world.comp::<Gang>(gang)?;
    if g.district_hideouts.is_empty() || !on(world) {
        return world.hideout_of(gang);
    }
    let from =
        crate::systems::contracts::home_door(world, id).or_else(|| world.comp::<Position>(id).map(|p| p.tile))?;
    nearest_to(world, gang, from)
}

use crate::components::TilePos;

/// The storey a Hideout takes on `host` (the upper floors): the highest storey above the ground no use stands on
/// (a Home host's flats count as used), else the top storey of a Home host's flats (its span then shrinks under
/// it: the second of the pair). `None`: no storey above the ground to take.
fn upper_storey(world: &World, host: EntityId, except: Option<EntityId>) -> Option<(u8, Option<(u8, u8)>)> {
    let hb = world.comp::<Building>(host)?;
    let mut taken = std::collections::BTreeSet::new();
    let mut top = hb.floors.saturating_sub(1);
    for t in crate::systems::floors::tenancies_of(world, host) {
        if Some(t) == except {
            continue;
        }
        if let Some((lo, hi)) = world.comp::<Building>(t).filter(|b| !b.demolished).and_then(|b| b.span) {
            taken.extend(lo..=hi);
            top = top.max(hi);
        }
    }
    let home = hb.kind == BuildingKind::Home;
    if let Some((lo, hi)) = hb.span {
        top = top.max(hi);
        if !home {
            taken.extend(lo..=hi);
        }
    }
    let flats = if home { hb.span } else { None };
    if let Some(f) = (1..=top).rev().find(|f| !taken.contains(f) && flats.is_none_or(|(lo, hi)| *f < lo || *f > hi)) {
        return Some((f, None));
    }
    match flats {
        Some((lo, hi)) if hi > lo && hi > 0 && !taken.contains(&hi) => Some((hi, Some((lo, hi - 1)))),
        _ => None,
    }
}

/// Move tenancy `t` up to its host's upper storey ([`upper_storey`]); the storey. `None`: it stays.
fn move_up(world: &mut World, t: EntityId) -> Option<u8> {
    let host = world.comp::<Building>(t)?.within?;
    let (f, shrink) = upper_storey(world, host, Some(t))?;
    if let Some(s) = shrink {
        if let Some(hb) = world.comp_mut::<Building>(host) {
            hb.span = Some(s);
        }
    }
    if let Some(b) = world.comp_mut::<Building>(t) {
        b.span = Some((f, f));
    }
    let inside = world.comp::<Building>(t).map(|b| b.occupants.clone()).unwrap_or_default();
    for o in inside {
        if let Some(p) = world.comp_mut::<Position>(o) {
            p.floor = f;
        }
    }
    Some(f)
}

/// At the world's build (before the bodies are put on their storeys): each gang's seat up to the top storey of
/// its host (`seat_up`).
pub fn seed(world: &mut World) {
    if !cfg(world).is_some_and(|c| c.seat_up) {
        return;
    }
    for g in world.gangs() {
        if let Some(h) = world.hideout_of(g) {
            move_up(world, h);
        }
    }
    crate::systems::floors::reindex(world);
}

/// The districts where `gang` holds a turf cell (turf off: of its held Homes), ascending.
fn held_districts(world: &World, gang: EntityId) -> Vec<crate::components::DistrictId> {
    let mut out: Vec<crate::components::DistrictId> = if crate::systems::turf::on(world) {
        world
            .turf
            .held
            .get(&gang)
            .map(|cells| cells.iter().map(|&k| world.district_of(crate::systems::turf::centre(world, k))).collect())
            .unwrap_or_default()
    } else {
        world
            .comp::<Gang>(gang)
            .map(|g| g.territory.iter().map(|&h| world.district_of_building(h)).collect())
            .unwrap_or_default()
    };
    out.sort_unstable();
    out.dedup();
    out
}

/// The point of `gang`'s turf in district `d` its new Hideout stands near: the mean of its held cells' centres
/// there (turf off: its held Homes' doors).
fn turf_centre(world: &World, gang: EntityId, d: crate::components::DistrictId) -> Option<TilePos> {
    let pts: Vec<TilePos> = if crate::systems::turf::on(world) {
        world
            .turf
            .held
            .get(&gang)?
            .iter()
            .map(|&k| crate::systems::turf::centre(world, k))
            .filter(|&t| world.district_of(t) == d)
            .collect()
    } else {
        let g = world.comp::<Gang>(gang)?;
        g.territory
            .iter()
            .filter_map(|&h| world.comp::<Building>(h).map(|b| b.door))
            .filter(|&t| world.district_of(t) == d)
            .collect()
    };
    if pts.is_empty() {
        return None;
    }
    let n = pts.len() as u32;
    let x = pts.iter().map(|t| u32::from(t.x)).sum::<u32>() / n;
    let y = pts.iter().map(|t| u32::from(t.y)).sum::<u32>() / n;
    Some(TilePos::new(x as u16, y as u16))
}

/// The front under a host's upper floors: a business on its ground storey (a venue first), the host's own when
/// it is one.
fn front_of(world: &World, host: EntityId) -> Option<EntityId> {
    let ground = |b: EntityId| {
        world.comp::<Building>(b).filter(|bd| !bd.demolished && bd.span.is_none_or(|(lo, _)| lo == 0)).map(|bd| bd.kind)
    };
    // A front: a venue or a shop (a kind a unit takes, `[tenancy] unit_kinds`), a Market.
    let business =
        |k: BuildingKind| k.is_leisure() || crate::systems::floors::unit_kind(world, k) || k == BuildingKind::Market;
    let mut cands: Vec<(bool, EntityId)> = crate::systems::floors::tenancies_of(world, host)
        .into_iter()
        .chain(std::iter::once(host))
        .filter_map(|b| ground(b).filter(|&k| business(k)).map(|k| (!k.is_leisure(), b)))
        .collect();
    cands.sort_unstable();
    cands.first().map(|x| x.1)
}

/// The host for `gang`'s new Hideout in district `d`: of the hosts there with a vacant unit and a storey above
/// the ground to take, those of `min_storeys` or more first, over a front first, nearest its turf's centre
/// (ties lower id).
fn pick_host(world: &World, gang: EntityId, d: crate::components::DistrictId, c: &VerticalCfg) -> Option<EntityId> {
    let at = turf_centre(world, gang, d)?;
    world
        .floor_index
        .vacant
        .keys()
        .copied()
        .filter_map(|h| {
            let b = world.comp::<Building>(h)?;
            if b.demolished || b.derelict || b.under_construction_until.is_some() || world.district_of(b.door) != d {
                return None;
            }
            upper_storey(world, h, None)?;
            let tall = b.span.map_or(b.floors, |(_, hi)| hi.max(b.floors)) >= c.min_storeys;
            Some((!tall, front_of(world, h).is_none(), b.door.manhattan(at), h))
        })
        .min()
        .map(|x| x.3)
}

/// Found `gang`'s Hideout on `host`: a tenancy on its upper storey, the key money paid to the landlord, a line.
fn found(world: &mut World, gang: EntityId, host: EntityId, c: &VerticalCfg) -> Option<EntityId> {
    let t = crate::systems::floors::found_unit(world, host, BuildingKind::Hideout, Some(gang))?;
    let f = move_up(world, t).unwrap_or(0);
    crate::systems::floors::reindex(world);
    let landlord = world.owner_of(host);
    let paid = if c.found_cost > 0 { ownership::pay(world, Some(gang), landlord, c.found_cost, Flow::Sale) } else { 0 };
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        if let Err(i) = g.district_hideouts.binary_search(&t) {
            g.district_hideouts.insert(i, t);
        }
    }
    ownership::jobs2_row(world).hideouts_founded += 1;
    crate::systems::virt::mark_dirty(world);
    let gname = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    let d = world.district_of_building(host);
    let over = front_of(world, host).map_or_else(String::new, |fr| format!(" over {}", world.name_of(fr)));
    let price = if paid >= c.found_cost {
        format!("{paid} key money to {}", world.owner_label(landlord))
    } else {
        format!("{paid} of {} key money to {} (all it had)", c.found_cost, world.owner_label(landlord))
    };
    let mut actors = vec![gang, t, host];
    // The members who live in the district hear of it (their nearest Hideout now).
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    actors.extend(members.into_iter().filter(|&m| own_hideout(world, m) == Some(t)).take(24));
    world.push_event(
        EventKind::Floor,
        &actors,
        format!(
            "{gname} took floor {f} of {}{over} as a Hideout in {} (its turf there): {price}",
            world.name_of(host),
            world.district_name(d)
        ),
    );
    Some(t)
}

/// Vertical turf (`gang::claim`): gang `gang`'s third blow on Home `home`, held by gang `holder`, of
/// `split_min_storeys` storeys or more: its upper half carved off as a Home tenancy (`within` its host, the span's
/// upper storeys, the share of its slots and storeys, its rent, its owner), the households whose flats stand there
/// moved into it, and the claim held on it; the lower floors stay the holder's. The Extortion line's suffix, or
/// `None` (the whole Home is taken as before).
pub fn split_claim(world: &mut World, home: EntityId, gang: EntityId, holder: EntityId) -> Option<String> {
    let min = cfg(world).map(|c| c.split_min_storeys).filter(|&m| m > 0)?;
    let hb = world.comp::<Building>(home)?;
    let (lo, hi) = hb.span?;
    let n = hi.saturating_sub(lo) + 1;
    if hb.kind != BuildingKind::Home || hb.demolished || n < min.max(2) {
        return None;
    }
    let mid = lo + n / 2;
    let up_n = hi - mid + 1;
    let share = |x: u16| -> u16 { ((u32::from(x) * u32::from(up_n)) / u32::from(n)).max(1) as u16 };
    let (rect, door, tier, owner, rent) = (hb.rect, hb.door, hb.tier, hb.owner, hb.rent_per_day);
    let cap = share(hb.capacity).min(hb.capacity.saturating_sub(1));
    let floors = (share(u16::from(hb.floors)) as u8).min(hb.floors.saturating_sub(1)).max(1);
    if cap == 0 {
        return None;
    }
    let host = crate::systems::floors::host_of(world, home);
    // The households upstairs move with their floors (by the storey their flat stands on now).
    let mut movers: Vec<EntityId> = {
        let orig = world.comp::<Building>(home)?;
        world
            .residents_of(home)
            .iter()
            .copied()
            .filter(|&a| {
                crate::systems::floors::flat_of(world, a)
                    .is_some_and(|f| crate::systems::floors::flat_floor(orig, home, f) >= mid)
            })
            .collect()
    };
    let mut b = Building::blank(BuildingKind::Home, rect, door, tier);
    b.within = Some(host);
    b.span = Some((mid, hi));
    b.capacity = cap;
    b.floors = floors;
    b.rent_per_day = rent;
    b.claim = Some(crate::components::Claim { gang, count: crate::systems::gang::CLAIM_HELD });
    let t = world.spawn();
    world.insert(t, b);
    let list = world.buildings_by_kind_mut().entry(BuildingKind::Home).or_default();
    if let Err(i) = list.binary_search(&t) {
        list.insert(i, t);
    }
    crate::systems::ownership::transfer_building(world, t, owner);
    if let Some(hb) = world.comp_mut::<Building>(home) {
        hb.span = Some((lo, mid - 1));
        hb.capacity = hb.capacity.saturating_sub(cap).max(1);
        hb.floors = hb.floors.saturating_sub(floors).max(1);
        hb.claim = Some(crate::components::Claim { gang: holder, count: crate::systems::gang::CLAIM_HELD });
    }
    movers.sort_unstable();
    movers.truncate(usize::from(cap));
    for &a in &movers {
        world.set_home(a, Some(t));
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        if let Err(i) = g.territory.binary_search(&t) {
            g.territory.insert(i, t);
        }
    }
    crate::systems::districts::rebuild(world);
    crate::systems::floors::reindex(world);
    ownership::jobs2_row(world).floor_claims += 1;
    let name = |g: EntityId| world.comp::<Gang>(g).map_or_else(String::new, |x| x.name.clone());
    let (gn, hn) = (name(gang), name(holder));
    let mut actors = vec![gang, holder, home, t];
    actors.extend(movers.iter().copied().take(12));
    world.push_event(
        EventKind::TerritoryFlipped,
        &actors,
        format!(
            "{gn} took floors {mid}-{hi} of Block#{} from {hn} ({} households upstairs): floors {lo}-{} stay {}",
            home.index,
            movers.len(),
            mid - 1,
            poss(&hn)
        ),
    );
    crate::systems::gang::push_shock(world, holder, crate::components::Shock::HomeFlippedAgainst);
    Some(format!(" (took floors {mid}-{hi} from {hn})"))
}

/// Midnight (the gangs' slot, after the turf's): the district Hideouts founded, the lifts held and let go, the
/// cuts ended, a stale climb cleared.
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    let now = world.tick;
    if crate::systems::floors::on(world) && c.found_per_day > 0 {
        for gang in world.gangs() {
            let Some(g) = world.comp::<Gang>(gang) else { continue };
            if g.members.is_empty() {
                continue;
            }
            let have: Vec<crate::components::DistrictId> =
                hideouts_of(world, gang).into_iter().map(|h| world.district_of_building(h)).collect();
            let want: Vec<crate::components::DistrictId> =
                held_districts(world, gang).into_iter().filter(|d| !have.contains(d)).collect();
            let mut n = 0;
            for d in want {
                if n >= c.found_per_day {
                    break;
                }
                if let Some(host) = pick_host(world, gang, d, &c) {
                    if found(world, gang, host, &c).is_some() {
                        n += 1;
                    }
                }
            }
        }
    }
    for gang in world.gangs() {
        let stale = world.comp::<Gang>(gang).is_some_and(|g| g.raid_climb.is_some() && g.raid_at.is_none());
        if stale {
            if let Some(g) = world.comp_mut::<Gang>(gang) {
                g.raid_climb = None;
            }
        }
    }
    // The cuts that end.
    let ended: Vec<EntityId> =
        world.lifts.iter().filter(|(_, s)| s.cut && s.cut_until.is_some_and(|t| t <= now)).map(|(&k, _)| k).collect();
    for core in ended {
        if let Some(s) = world.lifts.get_mut(&core) {
            s.cut = false;
            s.cut_until = None;
        }
        let text = format!("The lift of {} runs again", world.name_of(core));
        world.push_event(EventKind::Turf, &[core], text);
    }
    if c.hold_lifts {
        lift_holds(world);
    }
    let held = world.lifts.values().filter(|s| s.held_by.is_some()).count() as u32;
    let standing: u32 = world.gangs().iter().map(|&g| hideouts_of(world, g).len().saturating_sub(1) as u32).sum();
    let row = ownership::jobs2_row(world);
    row.lifts_held = held;
    row.hideouts_standing = standing;
}

/// The lifts held: each lift host a gang's Hideout stands in, held by the gang holding the cell of its door (of
/// the gangs with a Hideout there; none: let go). Each change a line to the gang and the tower.
pub fn lift_holds(world: &mut World) {
    let mut by_core: std::collections::BTreeMap<EntityId, Vec<EntityId>> = std::collections::BTreeMap::new();
    for gang in world.gangs() {
        for h in hideouts_of(world, gang) {
            let core = levels::core_of(world, h);
            if world.comp::<Building>(core).is_some_and(|b| b.lift.is_some()) {
                by_core.entry(core).or_default().push(gang);
            }
        }
    }
    // Lifts held by a gang with no Hideout there any more.
    let old: Vec<(EntityId, EntityId)> = world.lifts.iter().filter_map(|(&k, s)| s.held_by.map(|g| (k, g))).collect();
    let mut changes: Vec<(EntityId, Option<EntityId>, Option<EntityId>)> = Vec::new();
    for (core, g) in &old {
        if !by_core.contains_key(core) {
            changes.push((*core, Some(*g), None));
        }
    }
    for (core, gangs) in &by_core {
        let door = world.comp::<Building>(*core).map(|b| b.door);
        let holder = door.and_then(|d| crate::systems::turf::holder_at(world, d)).filter(|h| gangs.contains(h));
        let was = world.lifts.get(core).and_then(|s| s.held_by);
        if holder != was {
            changes.push((*core, was, holder));
        }
    }
    for (core, was, now) in changes {
        world.lifts.entry(core).or_default().held_by = now;
        let tower = world.name_of(core);
        let gname = |g: EntityId| world.comp::<Gang>(g).map_or_else(String::new, |x| x.name.clone());
        let (text, actors) = match (was, now) {
            (_, Some(g)) => {
                let from = was.map_or_else(String::new, |w| format!(" from {}", gname(w)));
                let mut a = vec![g, core];
                a.extend(was);
                a.extend(lift_members(world, core, g));
                (
                    format!(
                        "{} took the lift of {tower}{from}: its men ride first, everyone else waits or takes the stairs (they hold the street at the door)",
                        gname(g)
                    ),
                    a,
                )
            }
            (Some(w), None) => {
                let mut a = vec![w, core];
                a.extend(lift_members(world, core, w));
                (format!("{} let go of the lift of {tower}: it runs for anyone again", gname(w)), a)
            }
            (None, None) => continue,
        };
        if now.is_some() {
            ownership::jobs2_row(world).lift_holds += 1;
        }
        world.push_event(EventKind::Turf, &actors, text);
    }
}

/// The members of `gang` whose own Hideout climbs `core` (a line's actors: their diaries), 12 at most.
fn lift_members(world: &World, core: EntityId, gang: EntityId) -> Vec<EntityId> {
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    g.members
        .iter()
        .copied()
        .filter(|&m| own_hideout(world, m).is_some_and(|h| levels::core_of(world, h) == core))
        .take(12)
        .collect()
}

/// Cut the lift of `core` for `days` (0: nothing), naming who and why; the members of a gang holding it lose the
/// hold. True when it was cut now.
fn cut_lift(world: &mut World, core: EntityId, days: u16, what: &str, actors: &[EntityId]) -> bool {
    if days == 0 || world.comp::<Building>(core).is_none_or(|b| b.lift.is_none()) {
        return false;
    }
    let until = world.tick + Tick::from(days) * TICKS_PER_DAY;
    let s = world.lifts.entry(core).or_default();
    let fresh = !s.cut;
    s.cut = true;
    s.cut_until = Some(s.cut_until.map_or(until, |t| t.max(until)));
    if !fresh {
        return false;
    }
    ownership::jobs2_row(world).lift_cuts += 1;
    let mut a = actors.to_vec();
    a.push(core);
    let text = format!("{what}; the stairs for {days} days");
    world.push_event(EventKind::Turf, &a, text);
    true
}

/// A raid won or a Hideout sacked: the raiders cut the lift of the loser's host on the way out.
pub fn raid_cut(world: &mut World, winner: EntityId, loser: EntityId, at: EntityId) {
    let Some(days) = cfg(world).map(|c| c.raid_cut_days) else { return };
    let core = levels::core_of(world, at);
    let name = |g: EntityId| world.comp::<Gang>(g).map_or_else(String::new, |x| x.name.clone());
    let (w, l) = (name(winner), name(loser));
    let text = format!(
        "{w} cut the lift of {}: the cables of {} tower cut on the way out of the raid",
        world.name_of(core),
        poss(&l)
    );
    let mut actors = vec![winner, loser];
    actors.extend(lift_members(world, core, loser));
    cut_lift(world, core, days, &text, &actors);
}

/// A landlord short on a lift host's power (`industry::power`): the cars stop.
pub fn power_cut(world: &mut World, b: EntityId, owner: Option<EntityId>, paid: i64, due: i64) {
    let Some(days) = cfg(world).map(|c| c.power_cut_days) else { return };
    if paid >= due || world.comp::<Building>(b).is_none_or(|bd| bd.lift.is_none() || bd.within.is_some()) {
        return;
    }
    let text = format!(
        "The lift of {} stopped: its landlord {} came up short on the power ({paid} of {due})",
        world.name_of(b),
        world.owner_label(owner)
    );
    let actors: Vec<EntityId> = owner.into_iter().collect();
    cut_lift(world, b, days, &text, &actors);
}

/// A lift held by a gang: `who`'s wait in its lobby. `Some(0)`: a member rides first; `Some(extra)`: anyone else
/// waits this more. `None`: not held (or off).
pub fn held_wait(world: &World, core: EntityId, who: Option<EntityId>) -> Option<f64> {
    let g = world.lifts.get(&core)?.held_by?;
    let c = cfg(world)?;
    if who.is_some_and(|w| world.gang_of(w) == Some(g)) {
        return Some(0.0);
    }
    Some(f64::from(c.held_wait_ticks))
}

/// `name`'s possessive ("The Wire Hands'", "Ninefold's").
fn poss(name: &str) -> String {
    if name.ends_with('s') {
        format!("{name}'")
    } else {
        format!("{name}'s")
    }
}

/// The gang whose Hideout `b` is (its owner, else the gang seated or holed up there).
fn gang_at(world: &World, b: EntityId) -> Option<EntityId> {
    world
        .owner_of(b)
        .filter(|&o| world.has::<Gang>(o))
        .or_else(|| world.gang_list().iter().copied().find(|&g| is_hideout_of(world, g, b)))
}

/// The label of a raid's target: "the X Hideout" for a gang's, else its name.
fn place_of(world: &World, target: EntityId) -> String {
    if let Some(g) = gang_at(world, target) {
        if world.comp::<Building>(target).is_some_and(|b| b.kind == BuildingKind::Hideout) {
            let n = world.comp::<Gang>(g).map_or_else(String::new, |x| x.name.clone());
            return format!("the {} Hideout", n.strip_prefix("The ").unwrap_or(&n));
        }
    }
    world.name_of(target)
}

/// The raid's climb at the target's door (the Brawl step, after `raid::wait_for_crew`): true while the column
/// climbs (the step holds); false at once on the ground, off, or when it has reached the floor (the fight then
/// resolves). The first call starts it: by the lift or the stairs, the lobby and the floors passed told.
pub fn climb_column(world: &mut World, actor: EntityId) -> bool {
    let Some(c) = cfg(world).cloned() else { return false };
    if !levels::on(world) {
        return false;
    }
    let Some(crate::systems::raid::Expedition::Gang(gid)) = crate::systems::raid::expedition_of(world, actor) else {
        return false;
    };
    if world.comp::<Gang>(gid).is_none_or(|g| g.raid_at.is_none()) {
        return false;
    }
    let Some(target) = crate::systems::raid::gang_target(world, gid) else { return false };
    let Some((door, floor)) = world.comp::<Building>(target).map(|b| (b.door, b.span.map_or(0, |(lo, _)| lo))) else {
        return false;
    };
    if floor == 0 {
        return false;
    }
    let now = world.tick;
    if let Some(rc) = world.comp::<Gang>(gid).and_then(|g| g.raid_climb.clone()) {
        if rc.target == target {
            if now < rc.arrive {
                return true;
            }
            if let Some(g) = world.comp_mut::<Gang>(gid) {
                g.raid_climb = None;
            }
            return false;
        }
    }
    let members = world.comp::<Gang>(gid).map(|g| g.members.clone()).unwrap_or_default();
    let column = crate::systems::raid::gathered(world, &members, actor, door);
    let n = column.len();
    // Too few at the door: no climb (`raid::resolve` calls it off there).
    if n < world.config.gangs.raid_min_members {
        return false;
    }
    let core = levels::core_of(world, target);
    let rival = gang_at(world, target);
    let st = world.lifts.get(&core).cloned().unwrap_or_default();
    let has_lift = world.comp::<Building>(core).is_some_and(|b| b.lift.is_some());
    let gname = |g: EntityId| world.comp::<Gang>(g).map_or_else(String::new, |x| x.name.clone());
    let held_against = st.held_by.filter(|&h| h != gid);
    let stairs_why = if !has_lift {
        Some("no lift".to_string())
    } else if st.cut {
        Some("the lift cut".to_string())
    } else if let Some(h) = held_against {
        Some(format!("the lift held by {}", gname(h)))
    } else if n > usize::from(c.lift_max_raiders) {
        Some(format!("{n} of them, too many for the lobby"))
    } else {
        None
    };
    let plan = if stairs_why.is_none() {
        levels::plan(world, core, 0, floor, now, Some(actor))
    } else {
        levels::stairs(world, 0, floor)
    };
    let Some(plan) = plan else { return false };
    let (g, r) = (gname(gid), rival.map(gname));
    levels::book(world, core, plan, floor, now);
    let place = place_of(world, target);
    let host = crate::systems::floors::host_of(world, target);
    let tower = world.name_of(host);
    let row = ownership::jobs2_row(world);
    row.raid_climbs += 1;
    if plan.lift {
        row.raid_climb_lift += 1;
    }
    if let Some(gg) = world.comp_mut::<Gang>(gid) {
        gg.raid_climb = Some(RaidClimb {
            target,
            core,
            floor,
            start: now,
            board: now + plan.wait,
            arrive: now + plan.ticks,
            lift: plan.lift,
            column: column.clone(),
        });
    }
    let how = if plan.lift {
        format!(
            "rode the lift of {} up to {place} on floor {floor}",
            if host == target { "the building" } else { &tower }
        )
    } else {
        let why = stairs_why.unwrap_or_else(|| "the stairs quicker".to_string());
        let of = if host == target { place.clone() } else { tower.clone() };
        format!(
            "went up the stairs of {of} to {} on floor {floor} ({why})",
            if host == target { "its top" } else { &place }
        )
    };
    let mut actors = vec![gid];
    actors.extend(rival);
    actors.extend(column.iter().copied());
    world.push_event(EventKind::Raid, &actors, format!("{} raiders ({n}) {how}, {} min up", poss(&g), plan.ticks));
    hear(
        world,
        &ClimbHeard { gang: gid, rival, head: actor, host, door, floor, lift: plan.lift, column: &column, g: &g, r },
    );
    true
}

struct ClimbHeard<'a> {
    gang: EntityId,
    rival: Option<EntityId>,
    head: EntityId,
    host: EntityId,
    door: TilePos,
    floor: u8,
    lift: bool,
    column: &'a [EntityId],
    g: &'a str,
    r: Option<String>,
}

/// The bodies inside the structure on the floors the column climbs past hear it (by the stairs; by the lift, the
/// lobby sees it): a line to them (`hear_cap` at most, the nearest floors first) and a sighting of its head.
fn hear(world: &mut World, h: &ClimbHeard) {
    let cap = cfg(world).map_or(0, |c| usize::from(c.hear_cap));
    if cap == 0 {
        return;
    }
    let mut bs = crate::systems::floors::tenancies_of(world, h.host);
    bs.push(h.host);
    let mut near: Vec<(u8, EntityId)> = Vec::new();
    for b in bs {
        let Some(bd) = world.comp::<Building>(b) else { continue };
        for &o in &bd.occupants {
            if h.column.contains(&o) || world.gang_of(o) == Some(h.gang) {
                continue;
            }
            let f = world.comp::<Position>(o).map_or(0, |p| p.floor);
            let heard = if h.lift { f == 0 } else { f >= 1 && f < h.floor };
            if heard {
                near.push((f, o));
            }
        }
    }
    near.sort_unstable();
    near.dedup_by_key(|x| x.1);
    near.truncate(cap);
    if near.is_empty() {
        return;
    }
    let ids: Vec<EntityId> = near.iter().map(|x| x.1).collect();
    ownership::jobs2_row(world).raid_heard += ids.len() as u32;
    let tower = world.name_of(h.host);
    let whose = h.r.as_deref().map_or_else(|| "a floor".to_string(), |r| format!("{} floor", poss(r)));
    let text = if h.lift {
        format!(
            "In the lobby of {tower}: {} raiders ({}) packed into the lift for {whose} ({}), word of it out on the street",
            poss(h.g),
            h.column.len(),
            h.floor
        )
    } else {
        format!(
            "On the stairs of {tower}: {} raiders ({}) went up past the doors toward {whose} ({}), heard through the walls",
            poss(h.g),
            h.column.len(),
            h.floor
        )
    };
    let mut actors = ids.clone();
    actors.push(h.gang);
    actors.extend(h.rival);
    world.push_event(EventKind::Witness, &actors, text);
    for o in ids {
        crate::systems::gossip::maybe_sight(world, o, h.head, Some(h.host), h.door);
    }
}

/// Where a raider on his column's climb is (the view's): the target's door, the storey reached. `None`: on no
/// climb.
pub fn column_place(world: &World, id: EntityId) -> Option<(TilePos, EntityId, f32)> {
    let g = world.comp::<Gang>(world.gang_of(id)?)?;
    let rc = g.raid_climb.as_ref()?;
    if g.raid_at.is_none() || world.tick >= rc.arrive || !rc.column.contains(&id) {
        return None;
    }
    let door = world.comp::<Building>(rc.target)?.door;
    Some((door, rc.target, levels::storey_at(0, rc.floor, rc.start, rc.board, rc.arrive, world.tick)))
}

/// The guards answering an alarm at a target on `floor` climb to it: the walk to the door plus the climb by the
/// stairs within the answer's reach (`[crime] answer_radius` walked). Those who cannot make it, out.
pub fn climbing_guards(world: &World, target: EntityId, guards: &mut Vec<EntityId>) -> usize {
    let Some(floor) = world.comp::<Building>(target).and_then(|b| b.span).map(|(lo, _)| lo).filter(|&f| f > 0) else {
        return 0;
    };
    if !on(world) || !levels::on(world) {
        return 0;
    }
    let Some(door) = world.comp::<Building>(target).map(|b| b.door) else { return 0 };
    let climb = levels::stairs(world, 0, floor).map_or(0, |p| p.ticks);
    let reach = world.config.exec.walk_ticks(u64::from(world.config.crime.answer_radius));
    let before = guards.len();
    guards.retain(|&g| {
        world
            .comp::<Position>(g)
            .is_some_and(|p| world.config.exec.walk_ticks(u64::from(p.tile.manhattan(door))) + climb <= reach)
    });
    before - guards.len()
}

/// The run report's `#vertical` line: Hideouts per gang, the lifts held and cut.
pub fn report(world: &World) -> Vec<String> {
    if !on(world) {
        return Vec::new();
    }
    let gangs: Vec<EntityId> =
        world.gangs().into_iter().filter(|&g| world.comp::<Gang>(g).is_some_and(|x| !x.members.is_empty())).collect();
    let total: usize = gangs.iter().map(|&g| hideouts_of(world, g).len()).sum();
    let most = gangs.iter().map(|&g| hideouts_of(world, g).len()).max().unwrap_or(0);
    let up = gangs
        .iter()
        .flat_map(|&g| hideouts_of(world, g))
        .filter(|&h| world.comp::<Building>(h).and_then(|b| b.span).is_some_and(|(lo, _)| lo > 0))
        .count();
    let held = world.lifts.values().filter(|s| s.held_by.is_some()).count();
    let cut = world.lifts.values().filter(|s| s.cut).count();
    vec![format!(
        "#vertical\tgangs\t{}\thideouts\t{total}\tper_gang\t{:.2}\tmost\t{most}\tupstairs\t{up}\tlifts_held\t{held}\tlifts_cut\t{cut}",
        gangs.len(),
        total as f64 / gangs.len().max(1) as f64
    )]
}
