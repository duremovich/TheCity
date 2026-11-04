use super::*;

// ---------------------------------------------------------------------------
// Corpses as loot (plan D13-D15)
// ---------------------------------------------------------------------------

/// `kill_by` has put the Corpse on: file it for settlement, and settle at
/// once when the window is 0 or assets are off (M12: inheritance at death).
pub fn on_corpse(world: &mut World, c: EntityId) {
    if let Err(i) = world.loot_corpses.binary_search(&c) {
        world.loot_corpses.insert(i, c);
    }
    if !world.config.assets.enabled || world.config.assets.loot_window_hours == 0 {
        settle_corpse(world, c);
    }
}

/// Inheritance on what a corpse still carries (plan D13): its coins by the
/// M11 rule, carried packs to the coin heir (else the Recycler's stock,
/// the city's), goods into the heir's inventory up to capacity (the rest
/// is lost). Then `settled`; dropped from `loot_corpses`.
pub fn settle_corpse(world: &mut World, c: EntityId) {
    if let Ok(i) = world.loot_corpses.binary_search(&c) {
        world.loot_corpses.remove(i);
    }
    let Some(corpse) = world.comp_mut::<Corpse>(c) else { return };
    if corpse.settled {
        return;
    }
    corpse.settled = true;
    let loot = std::mem::take(&mut corpse.loot);
    if loot.coins > 0 {
        crate::systems::demography::inherit_coins(world, c, loot.coins);
    }
    let heir = crate::systems::demography::coin_heirs(world, c).first().copied().filter(|&h| world.has::<Brain>(h));
    let packs: Vec<EntityId> = assets_at(world, c)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.loc == AssetLoc::Carried(c)))
        .collect();
    for p in packs {
        match heir {
            Some(h) => {
                set_owner(world, p, Some(h));
                set_loc(world, p, AssetLoc::Carried(h));
            }
            None => {
                set_owner(world, p, None);
                // Jobs & lives phase 3: an heirless weapon goes to the Precinct.
                let weapon = world.comp::<Asset>(p).is_some_and(|x| x.kind == AssetKind::Weapon);
                let to = if weapon {
                    world.building_of_kind(BuildingKind::Jail)
                } else {
                    world.building_of_kind(BuildingKind::Cemetery)
                };
                match to {
                    Some(r) => set_loc(world, p, AssetLoc::Stock(r)),
                    None => despawn(world, p),
                }
            }
        }
    }
    if let Some(h) = heir {
        give_goods(world, h, loot.food, loot.stims, loot.parts);
    }
}

/// Goods into an agent's inventory up to its capacity (food also to
/// `FOOD_CAP`); the rest is lost.
pub fn give_goods(world: &mut World, agent: EntityId, food: u32, stims: u16, parts: u16) {
    let cap = capacity(world, agent);
    let Some(inv) = world.comp_mut::<Inventory>(agent) else { return };
    let mut room = cap.saturating_sub(load(inv));
    let f = food.min(room).min(FOOD_CAP.saturating_sub(inv.food));
    inv.food += f;
    room -= f;
    let s = u32::from(stims).min(room);
    inv.stims = inv.stims.saturating_add(s as u16);
    room -= s;
    let p = u32::from(parts).min(room / 3);
    inv.parts = inv.parts.saturating_add(p as u16);
}

/// Plan D14: the chrome in a buried body goes to the Recycler as Parts.
pub fn recycle_implants(world: &mut World, c: EntityId) {
    let implants: Vec<EntityId> = assets_at(world, c)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind.is_implant() && x.loc == AssetLoc::Installed(c)))
        .collect();
    if implants.is_empty() {
        return;
    }
    let per = world.config.assets.parts_per.implant;
    let recycler = world.building_of_kind(BuildingKind::Cemetery);
    for a in implants {
        if let Some(r) = recycler {
            world.add_stock(r, Good::Parts, per);
        }
        despawn(world, a);
    }
}

/// `demography::bury`: settle the body, then recycle its chrome.
pub fn on_buried(world: &mut World, c: EntityId) {
    settle_corpse(world, c);
    recycle_implants(world, c);
}

/// `World::remove_agent` (a freed corpse, an emigrant): an unsettled corpse
/// settles first; whatever is still at the entity goes with it.
pub fn on_removed(world: &mut World, id: EntityId) {
    if world.comp::<Corpse>(id).is_some_and(|c| !c.settled) {
        settle_corpse(world, id);
    }
    for a in assets_at(world, id).to_vec() {
        despawn(world, a);
    }
    world.trips.remove(&id);
    world.chase_pins.remove(&id);
    crate::systems::stims::end_deal(world, id);
}

/// Daily: corpses older than the window settle (so the window is 12–36 h:
/// daily granularity).
pub(super) fn settle_window(world: &mut World) {
    let window = u64::from(world.config.assets.loot_window_hours) * TICKS_PER_HOUR;
    let now = world.tick;
    for c in world.loot_corpses.clone() {
        let old = world.comp::<Corpse>(c).is_some_and(|k| now.saturating_sub(k.died_tick) >= window);
        if old {
            settle_corpse(world, c);
        }
    }
}

/// Plan D15: every unsettled, unstripped, unburied corpse rolls
/// `scav_strip_base × (2 − coverage)` on the world stream (ascending).
pub(super) fn scav_strip(world: &mut World) {
    let base = world.config.assets.scav_strip_base;
    for c in world.loot_corpses.clone() {
        let Some(k) = world.comp::<Corpse>(c) else { continue };
        if k.stripped || k.buried || k.settled {
            continue;
        }
        let Some(tile) = world.comp::<Position>(c).map(|p| p.tile) else { continue };
        let d = world.district_of(tile);
        let cover = world.district(d).coverage;
        let p = f64::from((base * (2.0 - cover)).clamp(0.0, 1.0));
        if world.rng.world().random_bool(p) {
            strip_offscreen(world, c);
        }
    }
}

/// An off-screen strip: in a gang-held district the loot goes to that
/// gang (coins to its treasury, goods and chrome to its Hideout, chrome
/// marked stolen); anywhere else goods and chrome are destroyed and the
/// coins go to the Treasury (destroying them would break M11 D3).
pub fn strip_offscreen(world: &mut World, c: EntityId) {
    let Some(tile) = world.comp::<Position>(c).map(|p| p.tile) else { return };
    let gang = match world.district(world.district_of(tile)).control {
        Controller::Gang(g) if world.has::<Gang>(g) => Some(g),
        _ => None,
    };
    let hideout = gang.and_then(|g| world.hideout_of(g));
    let Some(corpse) = world.comp_mut::<Corpse>(c) else { return };
    let loot = std::mem::take(&mut corpse.loot);
    corpse.stripped = true;
    match (gang, hideout) {
        (Some(g), Some(h)) => {
            loot_to_gang(world, g, loot.coins);
            world.add_stock(h, Good::Food, loot.food);
            world.add_stock(h, Good::Stims, u32::from(loot.stims));
            world.add_stock(h, Good::Parts, u32::from(loot.parts));
            let mut chrome = 0;
            for a in assets_at(world, c).to_vec() {
                chrome += usize::from(world.comp::<Asset>(a).is_some_and(|x| x.kind.is_implant()));
                into_gang_stock(world, a, g, h);
            }
            // Phase 3 (D35): the scavs' chrome is a harvest for the gang.
            if chrome > 0 {
                world.stats.current.harvests += 1;
                let (who, dead) = (world.owner_label(Some(g)), world.name_of(c));
                world.push_event(EventKind::Harvested, &[g, c], format!("{who} ripped {chrome} implants from {dead}"));
            }
        }
        _ => {
            if loot.coins > 0 {
                world.purse_add(None, loot.coins);
            }
            for a in assets_at(world, c).to_vec() {
                despawn(world, a);
            }
        }
    }
    let actor = gang.unwrap_or(EntityId::NONE);
    let (dead, by) = (world.name_of(c), gang.map_or_else(|| "scavengers".to_string(), |g| world.owner_label(Some(g))));
    world.push_event(
        EventKind::Stripped,
        &[actor, c],
        format!("{by} stripped the body of {dead} ({} coins)", loot.coins),
    );
    world.stats.current.stripped += 1;
    settle_corpse(world, c);
}

/// Loot coins (held by no purse) into a gang's treasury (gang income).
pub fn loot_to_gang(world: &mut World, gang: EntityId, coins: i64) {
    if coins > 0 {
        world.gang_credit(gang, coins);
    }
}

// ---------------------------------------------------------------------------
// The parts market (plan D14)
// ---------------------------------------------------------------------------

/// Every Clinic and Garage below `parts_floor` buys up to `parts_batch` at
/// `parts_price` (`Flow::Parts`), from gang Hideouts first (ascending gang
/// id), then the Recycler (revenue to the City).
pub fn parts_market(world: &mut World) {
    let (floor, batch, price) =
        (world.config.assets.parts_floor, world.config.assets.parts_batch, world.config.assets.parts_price.max(1));
    // Jobs & lives phase 3: a Gunsmith's Parts too (none stands with
    // `[weapons]` off).
    let mut buyers: Vec<EntityId> = world
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .chain(world.buildings_of_kind(BuildingKind::Garage))
        .chain(world.buildings_of_kind(BuildingKind::Gunsmith))
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .collect();
    buyers.sort_unstable();
    for b in buyers {
        if world.stock(b, Good::Parts) >= floor {
            continue;
        }
        let buyer = world.owner_of(b);
        let room = world.goods_cap(b, Good::Parts).saturating_sub(world.stock(b, Good::Parts));
        let mut want = batch.min(room);
        let mut sources: Vec<(Option<EntityId>, EntityId)> =
            world.gangs().into_iter().filter_map(|g| world.hideout_of(g).map(|h| (Some(g), h))).collect();
        // L2 L9: the Fabs after the gangs, the buyer's own corp's first (a
        // transfer to oneself moves no coin), then ascending id; none
        // stands with L2 off. L2 phase 5: the Recycler's scrap before a
        // rival's Fab (at one `parts_price` the city's scrap clears first;
        // with the Fabs first their surplus took every sale and the scrap
        // chain sold nothing on 42-47, 0 of ~950 scrap Parts).
        let mut fabs: Vec<(bool, EntityId)> = world
            .buildings_of_kind(BuildingKind::Fab)
            .iter()
            .copied()
            .filter(|&f| world.comp::<Building>(f).is_some_and(|bd| !bd.demolished && !bd.derelict))
            .map(|f| (world.owner_of(f) != buyer || buyer.is_none(), f))
            .collect();
        fabs.sort_unstable();
        let first_fab = sources.len();
        let own = fabs.iter().take_while(|(rival, _)| !rival).count();
        let rivals = fabs.split_off(own);
        sources.extend(fabs.into_iter().map(|(_, f)| (world.owner_of(f), f)));
        // Real economy E40: the work camps' Parts, after the buyer's own
        // Fabs and before the Recycler (none stands with `[camp]` off).
        for c in crate::systems::camp::standing_camps(world) {
            sources.push((world.owner_of(c), c));
        }
        if let Some(r) = world.building_of_kind(BuildingKind::Cemetery) {
            sources.push((None, r));
        }
        sources.extend(rivals.into_iter().map(|(_, f)| (world.owner_of(f), f)));
        for (i, (seller, src)) in sources.into_iter().enumerate() {
            if want == 0 {
                break;
            }
            let fab = i >= first_fab && world.comp::<Building>(src).is_some_and(|bd| bd.kind == BuildingKind::Fab);
            if seller == buyer && seller.is_some() && !fab {
                continue;
            }
            let have = world.stock(src, Good::Parts);
            let afford = match ownership::owner_kind(world, buyer) {
                OwnerKind::City | OwnerKind::Corp(_) => u32::MAX,
                _ => u32::try_from(world.purse(buyer).max(0) / price).unwrap_or(u32::MAX),
            };
            let n = want.min(have).min(afford);
            if n == 0 {
                continue;
            }
            let before = world.treasury().map_or(0, |t| t.coins);
            ownership::charge(world, buyer, seller, i64::from(n) * price, Flow::Parts);
            world.take_stock(src, Good::Parts, n);
            world.add_stock(b, Good::Parts, n);
            want -= n;
            // Real economy E33 (phase 3a): the Recycler's sale fills its till
            // with what the Treasury actually took (a city-owned buyer pays
            // the city itself: no coin moves, nothing reaches the till).
            if seller.is_none() && world.building_of_kind(BuildingKind::Cemetery) == Some(src) {
                let gained = world.treasury().map_or(0, |t| t.coins) - before;
                crate::systems::treasury::to_till(world, gained);
            }
            // L2 phase 5: the gate's probe (not a CSV column).
            match world.comp::<Building>(src).map(|bd| bd.kind) {
                Some(BuildingKind::Fab) => world.stats.current.living.parts_sold_fab += n,
                Some(BuildingKind::Cemetery) => world.stats.current.living.parts_sold_recycler += n,
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Appearance and the sales roll (plan D5, D17)
// ---------------------------------------------------------------------------

/// Plan D5, daily for every adult with a Body: dress from class and wealth,
/// chrome from `Kit.visible`, colours from the gang or the employer's corp.
pub(super) fn appearance(world: &mut World) {
    let execs = crate::systems::classes::exec_set(world);
    // scan-ok: daily: appearance
    for id in world.citizens() {
        if !world.has::<Body>(id) || !world.has::<Brain>(id) || !crate::systems::demography::is_adult(world, id) {
            continue;
        }
        let class = crate::systems::classes::class_in(world, id, &execs);
        let dress = match class {
            Class::Dreg => 0,
            Class::Corp => 3,
            Class::Street => {
                let spire = world
                    .comp::<crate::components::Household>(id)
                    .and_then(|h| h.home)
                    .and_then(|h| world.comp::<Building>(h))
                    .is_some_and(|b| b.tier >= 2);
                let rich = world.comp::<crate::components::Needs>(id).is_some_and(|n| n.wealth >= 0.8);
                if spire || rich {
                    2
                } else {
                    1
                }
            }
        };
        let chrome = world.comp::<Kit>(id).map_or(0, |k| k.visible);
        let colours = world
            .gang_of(id)
            .or_else(|| world.comp::<Job>(id).and_then(|j| j.employer).and_then(|e| world.corp_of_building(e)));
        // Jobs & lives phase 4 (§ 9): the worn clothing item's dress, when better.
        let dress = dress.max(crate::systems::fashion::worn_dress(world, id));
        // M15: a pinned dress (a god's suit) holds.
        let dress_pin = world.comp::<Appearance>(id).and_then(|a| a.dress_pin);
        let dress = dress_pin.unwrap_or(dress);
        let a = Appearance { dress, chrome, colours, dress_pin };
        if world.comp::<Appearance>(id) != Some(&a) {
            world.insert(id, a);
        }
    }
}

/// Roll each building's `asset_sales_today` into its 7-day window.
pub(super) fn roll_sales(world: &mut World) {
    for b in world.with::<Building>() {
        let Some(bd) = world.comp_mut::<Building>(b) else { continue };
        if bd.asset_sales_today == 0 && bd.asset_sales.is_empty() {
            continue;
        }
        let today = std::mem::take(&mut bd.asset_sales_today);
        bd.asset_sales.push_back(today);
        while bd.asset_sales.len() > SALES_DAYS {
            bd.asset_sales.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// Owners gone (plan D10, D45)
// ---------------------------------------------------------------------------

/// An owner died, left, dissolved or disbanded. An agent's parked vehicles,
/// posted robots and stock pass to its D45 heir (spouse, else first living
/// adult child, else the city); a financed asset in arrears goes to its
/// lender; carried and installed assets stay on the body (D13). A corp's or
/// gang's go to the city. Plans it lent on are re-pointed at the heir (an
/// agent) or the city. The vehicles it kept go home.
pub fn on_owner_gone(world: &mut World, gone: EntityId) {
    let agent = world.has::<Identity>(gone);
    let heir = if agent { agent_heir(world, gone) } else { None };
    for a in assets_of(world, Some(gone)).to_vec() {
        let Some(x) = world.comp::<Asset>(a).cloned() else { continue };
        // D45: a financed asset in arrears goes to its lender, even chrome
        // that stays in the body (phase 2 fix: the body skip ran first).
        let lender = x.finance.as_ref().filter(|f| f.arrears > 0).and_then(|f| f.lender);
        if let Some(l) = lender.filter(|&l| l != gone && live_owner(world, Some(l))) {
            take_back(world, a, l);
            continue;
        }
        if agent && matches!(x.loc, AssetLoc::Carried(h) | AssetLoc::Installed(h) | AssetLoc::InUse(h) if h == gone) {
            continue;
        }
        set_owner(world, a, heir);
    }
    let mut lent: Vec<EntityId> = Vec::new();
    let mut kept: Vec<EntityId> = Vec::new();
    for a in all_assets(world) {
        let Some(x) = world.comp::<Asset>(a) else { continue };
        if x.finance.as_ref().is_some_and(|f| f.lender == Some(gone)) {
            lent.push(a);
        }
        if x.keeper == Some(gone) {
            kept.push(a);
        }
    }
    for a in lent {
        if let Some(f) = world.comp_mut::<Asset>(a).and_then(|x| x.finance.as_mut()) {
            f.lender = heir;
        }
    }
    for a in kept {
        set_keeper(world, a, None);
        // Phase 3: an implant reserved in a Hideout's stock stays there.
        if world.comp::<Asset>(a).is_some_and(|x| !x.kind.is_vehicle()) {
            continue;
        }
        let home = world.comp::<Asset>(a).and_then(|x| x.owner).and_then(|o| world.hideout_of(o));
        let driven = world.comp::<Asset>(a).is_some_and(|x| matches!(x.loc, AssetLoc::InUse(_)));
        if let (Some(h), false) = (home, driven) {
            set_loc(world, a, AssetLoc::Parked(h));
        }
    }
}

/// M11 D45's heir: the living spouse, else the first living adult child.
fn agent_heir(world: &World, agent: EntityId) -> Option<EntityId> {
    let alive = |w: &World, id: EntityId| w.has::<Wallet>(id) && w.has::<Brain>(id) && !w.has::<Corpse>(id);
    world.spouse_of(agent).filter(|&s| s != agent && alive(world, s)).or_else(|| {
        crate::systems::demography::children_of_agent(world, agent)
            .into_iter()
            .filter(|&c| c != agent && alive(world, c) && crate::systems::demography::is_adult(world, c))
            .min()
    })
}
