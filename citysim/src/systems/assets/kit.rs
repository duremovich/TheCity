use super::*;

/// The Kit `agent` would have now, from its assets.
pub fn compute_kit(world: &World, agent: EntityId) -> Kit {
    let cfg = &world.config.chrome;
    let mut k = Kit::default();
    for &a in assets_at(world, agent) {
        let Some(x) = world.comp::<Asset>(a) else { continue };
        let AssetKind::Implant(slot) = x.kind else { continue };
        if x.loc != AssetLoc::Installed(agent) {
            continue;
        }
        let t = x.tier.max(1);
        k.chrome = true;
        // Jobs & lives phase 5 (§ 5.2): the behaviour chip's load is its own.
        k.load += if slot == Slot::Behaviour {
            world.config.vice.chip_load
        } else {
            cfg.sanity_cost.get(usize::from(t - 1)).copied().unwrap_or(0.0)
        };
        k.chrome_value += x.value;
        if matches!(slot, Slot::Arms | Slot::Eyes | Slot::Skin) {
            k.visible = k.visible.saturating_add(t);
        }
        if x.bricked || x.condition == 0 {
            continue;
        }
        // M14 V23: the effects run at the effective tier (the nominal one
        // above: load, value and what shows).
        let t = eff_tier_of(world, x).max(1);
        let tf = f32::from(t);
        match slot {
            Slot::Arms => {
                k.fighting += cfg.arms_fighting * tf;
                k.strength += cfg.arms_strength * tf;
            }
            Slot::Legs => {
                k.walk_mult = k.walk_mult.min(cfg.legs_walk.get(usize::from(t - 1)).copied().unwrap_or(1.0));
            }
            Slot::Nerves => k.reflex += cfg.nerves_reflex * tf,
            Slot::Eyes => {
                k.sight = k.sight.saturating_add(cfg.eyes_sight.saturating_mul(t));
                k.stealth += cfg.eyes_stealth * tf;
            }
            Slot::Skin => k.armour = k.armour.max(cfg.skin_armour * tf),
            // Its load only.
            Slot::Behaviour => {}
        }
    }
    // M14 V36: the deck carried (lowest id, condition > 0) at its effective tier.
    if let Some((d, x)) = assets_at(world, agent).iter().find_map(|&a| {
        world
            .comp::<Asset>(a)
            .filter(|x| x.kind == AssetKind::Deck && x.loc == AssetLoc::Carried(agent) && x.condition > 0)
            .map(|x| (a, x))
    }) {
        k.deck = Some(d);
        k.deck_tier = eff_tier_of(world, x).max(1);
    }
    // Jobs & lives phase 3 (§ 4): the best weapon carried (condition > 0)
    // at its effective tier. None exists with `[weapons]` off.
    // The combat model C1 (§ 4): and its kind (`Asset.wkind`, else mapped
    // for a civilian by its tier). The best: the tier, then the lower id.
    let best = assets_at(world, agent)
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a).map(|x| (a, x)))
        .filter(|(_, x)| x.kind == AssetKind::Weapon && x.loc == AssetLoc::Carried(agent) && x.condition > 0)
        .map(|(a, x)| (eff_tier_of(world, x).max(1), std::cmp::Reverse(a), x.wkind, x.tier))
        .max();
    if let Some((t, std::cmp::Reverse(a), wk, tier)) = best {
        k.weapon = t;
        k.weapon_kind = if wk > 0 {
            wk - 1
        } else {
            crate::systems::combat::map_kind(world, tier, "civil", u64::from(a.index)).saturating_sub(1)
        };
    }
    // C1 (§ 2): a leg wound slows the walk (x the step cost).
    let limp = crate::systems::health::effects(world, agent).walk_mult;
    if limp != 1.0 {
        k.walk_mult *= limp;
    }
    // M14 V31: else the fleet deck a corp's run order lends at its Lab chair.
    if k.deck.is_none() {
        let lent = world
            .run_orders
            .get(&agent)
            .filter(|o| o.why == crate::virt::RunWhy::CorpOrder)
            .and_then(|o| crate::systems::virt::fleet_deck_at(world, o.chair));
        if let Some((d, x)) = lent.and_then(|d| world.comp::<Asset>(d).map(|x| (d, x))) {
            k.deck = Some(d);
            k.deck_tier = eff_tier_of(world, x).max(1);
        }
    }
    k.vehicle = vehicle_of(world, agent);
    k.driving = world.trips.get(&agent).and_then(|t| world.comp::<Asset>(t.vehicle)).map(|x| x.kind);
    let vehicle_tier = k.vehicle.and_then(|v| world.comp::<Asset>(v)).map_or(0, |x| {
        if x.kind == AssetKind::Flyer {
            3
        } else {
            // M14 V23: the vehicle's effective tier.
            eff_tier_of(world, x)
        }
    });
    k.flash = ((f32::from(k.visible) + f32::from(vehicle_tier)) / 9.0).min(1.0);
    // D21: the Coarse multiplier of the vehicle the agent would drive (the
    // trip's own kind is read at the trip: `vehicles::timed_mult`).
    if let Some(kind) = k.vehicle.and_then(|v| world.comp::<Asset>(v)).map(|x| x.kind).filter(|k| k.is_road_vehicle()) {
        k.timed_mult = crate::systems::vehicles::timed_mult_of(world, kind);
    }
    k
}

/// Recompute an agent's Kit (a no-op for anything without one).
pub fn rekit(world: &mut World, agent: EntityId) {
    if !world.has::<Kit>(agent) {
        return;
    }
    let k = compute_kit(world, agent);
    if let Some(slot) = world.comp_mut::<Kit>(agent) {
        *slot = k;
    }
}

/// The agent's Kit, or the bare default.
pub fn kit(world: &World, agent: EntityId) -> Kit {
    world.comp::<Kit>(agent).cloned().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Places
// ---------------------------------------------------------------------------

/// Where an asset stands: its building's door, or its holder agent's tile.
pub fn asset_tile(world: &World, a: EntityId) -> Option<TilePos> {
    let holder = world.comp::<Asset>(a)?.loc.holder()?;
    world.comp::<Building>(holder).map(|b| b.door).or_else(|| world.comp::<Position>(holder).map(|p| p.tile))
}

/// The building of `kind` nearest `from` (door Manhattan, ties lower id),
/// standing and not derelict, optionally owned by `owner` only.
pub(crate) fn nearest_building(
    world: &World,
    kind: BuildingKind,
    from: TilePos,
    owner: Option<Option<EntityId>>,
) -> Option<EntityId> {
    world
        .buildings_of_kind(kind)
        .iter()
        .filter_map(|&b| {
            let bd = world.comp::<Building>(b).filter(|bd| !bd.demolished && !bd.derelict)?;
            if owner.is_some_and(|o| bd.owner != o) {
                return None;
            }
            Some((bd.door.manhattan(from), b))
        })
        .min()
        .map(|(_, b)| b)
}

/// The building whose door is nearest `from` (any kind, standing).
pub(super) fn nearest_any_building(world: &World, from: TilePos) -> Option<EntityId> {
    world
        .buildings_by_kind()
        .values()
        .flatten()
        .filter_map(|&b| world.comp::<Building>(b).filter(|bd| !bd.demolished).map(|bd| (bd.door.manhattan(from), b)))
        .min()
        .map(|(_, b)| b)
}

/// An owner whose purse is live: the city, a corp, a gang or a living agent.
pub(super) fn live_owner(world: &World, owner: Option<EntityId>) -> bool {
    match owner {
        None => true,
        Some(o) => world.has::<Corp>(o) || world.has::<Gang>(o) || world.has::<Wallet>(o),
    }
}

pub(super) fn is_agent(world: &World, owner: Option<EntityId>) -> Option<EntityId> {
    owner.filter(|&o| world.has::<Identity>(o) && world.has::<Wallet>(o))
}

/// What a purse can pay now: a corp or the city anything, an agent or gang its coins.
pub(super) fn can_pay(world: &World, owner: Option<EntityId>, amount: i64) -> bool {
    match ownership::owner_kind(world, owner) {
        OwnerKind::City | OwnerKind::Corp(_) => true,
        OwnerKind::Gang(_) | OwnerKind::Agent(_) => world.purse(owner) >= amount,
    }
}

// ---------------------------------------------------------------------------
// Goods stock (plan D6)
// ---------------------------------------------------------------------------

impl World {
    /// A building's cap for `good`: Food as today (the kind's `stock_cap`);
    /// Stims and Parts at least `[assets] goods_cap`.
    pub fn goods_cap(&self, b: EntityId, good: Good) -> u32 {
        let Some(bd) = self.comp::<Building>(b) else { return 0 };
        let cap = self.config.buildings.for_kind(bd.kind).stock_cap;
        match good {
            Good::Food => cap,
            Good::Stims | Good::Parts => cap.max(self.config.assets.goods_cap),
            // Jobs & lives phase 1: the appended goods hold `[jobs2] goods_cap`.
            Good::Rations | Good::Braindance | Good::Organs => cap.max(self.config.jobs2.goods_cap),
        }
    }

    /// Units of `good` a building holds.
    pub fn stock(&self, b: EntityId, good: Good) -> u32 {
        self.comp::<Building>(b).map_or(0, |bd| match good {
            Good::Food => bd.stock_food,
            Good::Stims | Good::Parts => bd.stock_goods[good as usize - 1],
            Good::Rations | Good::Braindance | Good::Organs => bd.stock_x[good as usize - 3],
        })
    }

    /// Add up to `n` units, capped at `goods_cap`; returns the units added.
    pub fn add_stock(&mut self, b: EntityId, good: Good, n: u32) -> u32 {
        let cap = self.goods_cap(b, good);
        let Some(bd) = self.comp_mut::<Building>(b) else { return 0 };
        let slot = match good {
            Good::Food => &mut bd.stock_food,
            Good::Stims | Good::Parts => &mut bd.stock_goods[good as usize - 1],
            Good::Rations | Good::Braindance | Good::Organs => &mut bd.stock_x[good as usize - 3],
        };
        let added = n.min(cap.saturating_sub(*slot));
        *slot += added;
        added
    }

    /// Take up to `n` units; returns the units taken.
    pub fn take_stock(&mut self, b: EntityId, good: Good, n: u32) -> u32 {
        let Some(bd) = self.comp_mut::<Building>(b) else { return 0 };
        let slot = match good {
            Good::Food => &mut bd.stock_food,
            Good::Stims | Good::Parts => &mut bd.stock_goods[good as usize - 1],
            Good::Rations | Good::Braindance | Good::Organs => &mut bd.stock_x[good as usize - 3],
        };
        let taken = n.min(*slot);
        *slot -= taken;
        taken
    }
}
