//! Round 6 V1 (OPEN-231, Shadow V14): the household's shelf in a shared
//! Home.
//!
//! A Home's pantry is one `Building.stock_food`. At 100k a Block holds up
//! to ten families, and the children's meal, the grocery runs and the
//! service's warning all read the Block's stock: a broke family's run was
//! never made while the neighbours' groceries stood on the shelf, its
//! children ate them, and the second unfed spell the service's ladder waits
//! for never came (r5 100k seed 42 x 30: of the children warned at a door of
//! ten or more, 0.5 % were taken later, against 27.6 % at a door of one or
//! two); when the Block did run dry, every family's children were warned in
//! one line (`owner_broke_6426 d21 00:10`, eighteen children).
//!
//! The households under one roof are the kin living there (spouses,
//! Parent and Family edges; grouped at midnight, `daily`). Each adult keeps
//! a stake: the units on the shelf that are his. A unit he stores
//! (`StoreFood`, Full and off screen) is his; a unit an adult eats from the
//! shelf comes off his household's stake, and with none left off the
//! neighbours' (a neighbour's crust is eaten); his children's meal comes off
//! his household's stake; a move leaves the stake behind; at midnight the
//! stakes are scaled down to the stock (spoilage, a theft, a raid). Units
//! nobody holds (the seeded stock, a stake left behind) are common. In a
//! shared Home (two households or more, a stake on the shelf) a household's
//! claim is its stake and the common units; the children's meal, the
//! grocery runs, the warning and the let-home read it, and each household
//! has a Home pantry's `stock_cap` of room. A Home of one household, or one
//! with no stake on it, keeps the common shelf: as before.

use crate::components::{Building, BuildingKind, Household, RelKind};
use crate::entity::EntityId;
use crate::world::World;

/// `id`'s household key: the lowest id of his kin under his roof, else his own.
pub fn key(world: &World, id: EntityId) -> EntityId {
    world.comp::<Household>(id).and_then(|h| h.kin).unwrap_or(id)
}

fn stake(world: &World, id: EntityId) -> f32 {
    world.comp::<Household>(id).map_or(0.0, |h| h.pantry_stake.max(0.0))
}

/// The households under `home`'s roof: `(key, stake, members)`, ascending key.
fn households(world: &World, home: EntityId) -> Vec<(EntityId, f32, u32)> {
    let mut out: Vec<(EntityId, f32, u32)> = Vec::new();
    for &r in world.residents_of(home) {
        let (k, s) = (key(world, r), stake(world, r));
        match out.binary_search_by_key(&k, |h| h.0) {
            Ok(i) => {
                out[i].1 += s;
                out[i].2 += 1;
            }
            Err(i) => out.insert(i, (k, s, 1)),
        }
    }
    out
}

/// A household's claim from its stake, every stake and the stock: its
/// stake and the common units; with the stakes above the stock (taken
/// since midnight), its share of the stock.
fn claim_of(mine: f32, total: f32, stock: f32) -> f32 {
    if total <= stock {
        mine + (stock - total)
    } else {
        stock * mine / total
    }
}

/// The shelf of `home` as household `k` reads it: `Some((claim, members))`
/// in a shared Home with a stake on it, `None` when the shelf is common
/// (one household, or nobody's stake).
fn read(world: &World, home: EntityId, k: EntityId) -> Option<(f32, u32)> {
    let hs = households(world, home);
    if hs.len() < 2 {
        return None;
    }
    let total: f32 = hs.iter().map(|h| h.1).sum();
    if total <= 0.0 {
        return None;
    }
    let stock = world.comp::<Building>(home).map_or(0, |b| b.stock_food) as f32;
    let (mine, members) = hs.iter().find(|h| h.0 == k).map_or((0.0, 0), |h| (h.1, h.2));
    Some((claim_of(mine, total, stock), members))
}

/// What the other households under `home`'s roof hold above their
/// children's food (`ages::children_food_need_of`'s days): `(key, spare)`
/// for each household but `k`, its stake's units only (the common units are
/// in `k`'s own claim). A neighbour's crust is eaten; his child's meal is not.
fn others_spare(world: &World, home: EntityId, k: EntityId) -> Vec<(EntityId, f32)> {
    use crate::components::{Child, Routine};
    let mut hs: Vec<(EntityId, f32, u32)> = Vec::new();
    for &r in world.residents_of(home) {
        let (kk, s) = (key(world, r), stake(world, r));
        let kid =
            u32::from(world.comp::<Child>(r).is_some_and(|c| !matches!(c.routine, Routine::Camp | Routine::Street)));
        match hs.binary_search_by_key(&kk, |h| h.0) {
            Ok(i) => {
                hs[i].1 += s;
                hs[i].2 += kid;
            }
            Err(i) => hs.insert(i, (kk, s, kid)),
        }
    }
    let total: f32 = hs.iter().map(|h| h.1).sum();
    let stock = world.comp::<Building>(home).map_or(0, |b| b.stock_food) as f32;
    let per = if crate::systems::ages::on(world) {
        crate::systems::demography::CHILD_FOOD_PER_DAY * world.config.ages.child_pantry_days
    } else {
        0.0
    };
    hs.into_iter()
        .filter(|h| h.0 != k)
        .map(|(kk, s, kids)| {
            let held = if total > stock && total > 0.0 { s * stock / total } else { s };
            (kk, (held - (per * kids as f32).ceil()).max(0.0))
        })
        .collect()
}

/// The pantry units adult `id` may eat at `home` in a shared Home: his
/// household's claim and the other households' spare (`others_spare`);
/// `None` when the shelf is common.
pub fn adult_reach(world: &World, id: EntityId, home: EntityId) -> Option<f32> {
    let k = key(world, id);
    let own = claim(world, home, k)?;
    Some(own + others_spare(world, home, k).iter().map(|s| s.1).sum::<f32>())
}

/// Household `k`'s claim on `home`'s pantry, in units; `None`: the shelf is common.
pub fn claim(world: &World, home: EntityId, k: EntityId) -> Option<f32> {
    read(world, home, k).map(|r| r.0)
}

/// The households under `home`'s roof (the shadow's `[family]` line).
pub fn household_count(world: &World, home: EntityId) -> usize {
    households(world, home).len()
}

/// The pantry units `id`'s household holds at `home`: its claim (whole
/// units), else the whole stock.
pub fn held_by(world: &World, id: EntityId, home: EntityId) -> u32 {
    match claim(world, home, key(world, id)) {
        Some(c) => (c + 1e-3).floor() as u32,
        None => world.comp::<Building>(home).map_or(0, |b| b.stock_food),
    }
}

/// The residents `id`'s grocery run feeds at `home`: his household in a
/// shared Home, else every resident.
pub fn members(world: &World, id: EntityId, home: EntityId) -> u32 {
    match read(world, home, key(world, id)) {
        Some((_, m)) => m.max(1),
        None => world.residents_of(home).len().max(1) as u32,
    }
}

/// The children's food `id`'s household keeps at `home`
/// (`ages::children_food_need` over its children in a shared Home, the
/// Home's otherwise).
pub fn kids_need(world: &World, id: EntityId, home: EntityId) -> u32 {
    let k = key(world, id);
    if read(world, home, k).is_none() {
        return crate::systems::ages::children_food_need(world, home);
    }
    crate::systems::ages::children_food_need_of(world, home, Some(k))
}

/// The room on `id`'s household's shelf at `home` under a pantry's `cap`:
/// the Home's room, or in a shared Home its own `cap` less its stake (the
/// Block's shelf holds `cap` a household).
pub fn room(world: &World, id: EntityId, home: EntityId, cap: u32) -> u32 {
    let stock = world.comp::<Building>(home).map_or(0, |b| b.stock_food);
    let hs = households(world, home);
    if hs.len() < 2 {
        return cap.saturating_sub(stock);
    }
    let k = key(world, id);
    let mine = hs.iter().find(|h| h.0 == k).map_or(0.0, |h| h.1).floor() as u32;
    cap.saturating_sub(mine).min((cap * hs.len() as u32).saturating_sub(stock))
}

/// `StoreFood` (Full and off screen): `units` put on the shelf by `id`.
pub fn stored(world: &mut World, id: EntityId, units: u32) {
    if units == 0 {
        return;
    }
    if let Some(h) = world.comp_mut::<Household>(id) {
        h.pantry_stake += units as f32;
    }
}

/// The children's meal (`demography::children`) in a shared Home: `kids`
/// grouped by household, each with its claim, ascending key; `None` when
/// the shelf is common.
pub fn meal_groups(world: &World, home: EntityId, kids: &[EntityId]) -> Option<Vec<(EntityId, Vec<EntityId>, f32)>> {
    let hs = households(world, home);
    if hs.len() < 2 {
        return None;
    }
    let total: f32 = hs.iter().map(|h| h.1).sum();
    if total <= 0.0 {
        return None;
    }
    let stock = world.comp::<Building>(home).map_or(0, |b| b.stock_food) as f32;
    let mut out: Vec<(EntityId, Vec<EntityId>, f32)> = Vec::new();
    for &c in kids {
        let k = key(world, c);
        match out.binary_search_by_key(&k, |g| g.0) {
            Ok(i) => out[i].1.push(c),
            Err(i) => {
                let mine = hs.iter().find(|h| h.0 == k).map_or(0.0, |h| h.1);
                out.insert(i, (k, vec![c], claim_of(mine, total, stock)));
            }
        }
    }
    Some(out)
}

/// Before the children's meal (`demography::children`): a free parent of
/// `kids` living at `home` puts the food he carries (not stolen) on the
/// shelf, up to their meal tonight that his household's claim (the Home's
/// stock, with the meal's debt, when the shelf is common) does not cover:
/// the child eats from his pocket (`camp_parent` d24: a father on the
/// service's warning bought three meals for himself, ate one, carried two,
/// and his infant went unfed beside them). The units moved.
pub fn pocket_to_shelf(world: &mut World, home: EntityId, kids: &[EntityId]) -> u32 {
    use crate::systems::demography::CHILD_FOOD_PER_DAY;
    let (stock, debt) = world.comp::<Building>(home).map_or((0, 0.0), |b| (b.stock_food, b.child_food_debt));
    // (household key or None when common, its children, the units short).
    let groups: Vec<(Option<EntityId>, Vec<EntityId>, u32)> = match meal_groups(world, home, kids) {
        Some(gs) => gs
            .into_iter()
            .map(|(k, ks, claim)| {
                let short = (CHILD_FOOD_PER_DAY * ks.len() as f32 - claim).max(0.0).ceil() as u32;
                (Some(k), ks, short)
            })
            .collect(),
        None => {
            let want = (debt + CHILD_FOOD_PER_DAY * kids.len() as f32).floor() as u32;
            vec![(None, kids.to_vec(), want.saturating_sub(stock))]
        }
    };
    let mut moved = 0u32;
    for (k, ks, mut short) in groups {
        if short == 0 {
            continue;
        }
        let mut parents: Vec<EntityId> = ks
            .iter()
            .flat_map(|&c| crate::systems::camp::parents_of(world, c))
            .filter(|&p| {
                crate::systems::camp::free_parent(world, p)
                    && world.comp::<Household>(p).and_then(|h| h.home) == Some(home)
                    && k.is_none_or(|k| key(world, p) == k)
            })
            .collect();
        parents.sort_unstable();
        parents.dedup();
        for p in parents {
            if short == 0 {
                break;
            }
            let Some(inv) = world.comp_mut::<crate::components::Inventory>(p) else { continue };
            let m = inv.food.saturating_sub(inv.stolen_food).min(short);
            if m == 0 {
                continue;
            }
            let synth = inv.synth_held().min(m);
            inv.synth = inv.synth_held() - synth;
            inv.food -= m;
            inv.synth = inv.synth.min(inv.food);
            if let Some(b) = world.comp_mut::<Building>(home) {
                b.synth_food = b.synth_food.min(b.stock_food) + synth;
                b.stock_food += m;
            }
            stored(world, p, m);
            short -= m;
            moved += m;
        }
    }
    moved
}

/// Take `units` off the stakes of `who` (residents), each by his share.
/// The units not covered.
fn debit(world: &mut World, who: &[EntityId], units: f32) -> f32 {
    let held: Vec<(EntityId, f32)> = who.iter().map(|&r| (r, stake(world, r))).filter(|&(_, s)| s > 0.0).collect();
    let total: f32 = held.iter().map(|a| a.1).sum();
    if total <= 0.0 || units <= 0.0 {
        return units.max(0.0);
    }
    let f = (units / total).min(1.0);
    for (a, s) in held {
        if let Some(h) = world.comp_mut::<Household>(a) {
            h.pantry_stake = (s - s * f).max(0.0);
        }
    }
    (units - total).max(0.0)
}

/// `units` eaten from `home`'s shelf by household `k`'s children: off its
/// stake (the rest from the common units).
pub fn eaten(world: &mut World, home: EntityId, k: EntityId, units: f32) {
    let own: Vec<EntityId> = world.residents_of(home).iter().copied().filter(|&r| key(world, r) == k).collect();
    debit(world, &own, units);
}

/// An adult `id` ate `units` from `home`'s shelf (`EatAtHome`, `PackMeal`,
/// the off-screen meal), in this order: his household's units above its
/// children's food, the common units, the neighbours' spare above theirs
/// (`others_spare`, each by its share), his own children's food last (a
/// parent who ate his child's share first left the child unfed beside the
/// neighbours' crusts).
pub fn ate(world: &mut World, id: EntityId, home: EntityId, units: f32) {
    if world.residents_of(home).len() < 2 {
        return;
    }
    let k = key(world, id);
    let (own, others): (Vec<EntityId>, Vec<EntityId>) =
        world.residents_of(home).iter().copied().partition(|&r| key(world, r) == k);
    let kids = crate::systems::ages::children_food_need_of(world, home, Some(k)) as f32;
    let held: f32 = own.iter().map(|&r| stake(world, r)).sum();
    let mut left = units - (held - kids).max(0.0).min(units);
    debit(world, &own, units - left);
    if left <= 0.0 {
        return;
    }
    // The common units are what the shelf holds above every stake after
    // the meal: only what they did not cover comes off a stake.
    let stock = world.comp::<Building>(home).map_or(0, |b| b.stock_food) as f32;
    let total: f32 = world.residents_of(home).iter().map(|&r| stake(world, r)).sum();
    if total <= stock {
        return;
    }
    left = left.min(total - stock);
    let spare = others_spare(world, home, k);
    let sum: f32 = spare.iter().map(|s| s.1).sum();
    let from_spare = left.min(sum);
    if from_spare > 0.0 {
        for (kk, sp) in spare {
            if sp <= 0.0 {
                continue;
            }
            let who: Vec<EntityId> = others.iter().copied().filter(|&r| key(world, r) == kk).collect();
            debit(world, &who, from_spare * sp / sum);
        }
    }
    left -= from_spare;
    if left > 0.0 {
        let rest = debit(world, &own, left);
        debit(world, &others, rest);
    }
}

/// Midnight (`demography::early`, before the children's meal): each Home's
/// residents grouped into households (union-find over Spouse, Parent and
/// Family edges between residents; the key the lowest id), and the stakes
/// scaled down to the stock where units left the shelf since (spoilage, a
/// theft, a raid).
pub fn daily(world: &mut World) {
    let mut keys: Vec<(EntityId, Option<EntityId>)> = Vec::new();
    let mut scale: Vec<(EntityId, f32)> = Vec::new();
    let homes = world.buildings_by_kind().get(&BuildingKind::Home).cloned().unwrap_or_default();
    for home in homes {
        let mut rs: Vec<EntityId> = world.residents_of(home).to_vec();
        rs.sort_unstable();
        let total: f32 = rs.iter().map(|&r| stake(world, r)).sum();
        let stock = world.comp::<Building>(home).map_or(0, |b| b.stock_food) as f32;
        if total > stock {
            let f = stock / total;
            scale.extend(rs.iter().map(|&r| (r, f)));
        }
        if rs.len() < 2 {
            keys.extend(rs.into_iter().map(|r| (r, None)));
            continue;
        }
        let mut root: Vec<usize> = (0..rs.len()).collect();
        fn find(root: &mut [usize], mut i: usize) -> usize {
            while root[i] != i {
                root[i] = root[root[i]];
                i = root[i];
            }
            i
        }
        for (i, &r) in rs.iter().enumerate() {
            let kin: Vec<EntityId> = world
                .kin_candidates(r)
                .filter(|&o| {
                    world
                        .edge(r, o)
                        .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Parent | RelKind::Family))
                })
                .chain(world.spouse_of(r))
                .collect();
            for o in kin {
                if let Ok(j) = rs.binary_search(&o) {
                    let (a, b) = (find(&mut root, i), find(&mut root, j));
                    if a != b {
                        // The lower index (the lower id: `rs` is sorted) roots.
                        root[a.max(b)] = a.min(b);
                    }
                }
            }
        }
        for i in 0..rs.len() {
            let k = rs[find(&mut root, i)];
            keys.push((rs[i], (k != rs[i]).then_some(k)));
        }
    }
    for (r, k) in keys {
        if let Some(h) = world.comp_mut::<Household>(r) {
            h.kin = k;
        }
    }
    for (r, f) in scale {
        if let Some(h) = world.comp_mut::<Household>(r) {
            h.pantry_stake *= f;
        }
    }
}
