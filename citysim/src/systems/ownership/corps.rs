use super::*;

/// Coins in every purse: wallets, gang and corp treasuries, the Treasury,
/// (M13 D13) the coins on unsettled corpses, (M16a C5) the coins held in
/// contract records' escrow and (Real economy E25a, market on) a robbery's
/// loot in flight on an open hole.
pub fn total_coins(world: &World) -> i64 {
    let wallets: i64 = world.with::<Wallet>().iter().filter_map(|&a| world.comp::<Wallet>(a)).map(|w| w.coins).sum();
    let gangs: i64 = world.gangs().iter().filter_map(|&g| world.comp::<Gang>(g)).map(|g| g.treasury).sum();
    let corps: i64 = world.corps().iter().filter_map(|&c| world.comp::<Corp>(c)).map(|c| c.treasury).sum();
    let city: i64 = world.with::<Treasury>().iter().filter_map(|&t| world.comp::<Treasury>(t)).map(|t| t.coins).sum();
    let loot: i64 = world.loot_corpses.iter().filter_map(|&c| world.comp::<Corpse>(c)).map(|c| c.loot.coins).sum();
    let escrow: i64 = world.contracts.values().map(|c| c.escrow).sum();
    // Real economy (plan E25a): a robbery's coins in flight on an open hole
    // are coins held (as escrow is).
    let holes: i64 = world.holes.values().map(|h| h.loot).sum();
    // Real economy E26: the Missions' purses are coins held.
    let purses: i64 = crate::systems::charity::purses(world);
    // Real economy E33 (phase 3a): the Recycler's till (0 under the levy).
    let till = world.econ.recycler_till;
    wallets + gangs + corps + city + loot + escrow + holes + purses + till
}

// ---------------------------------------------------------------------------
// Buildings, values, roles
// ---------------------------------------------------------------------------

/// D28: what a building of `kind` sells for; 0 = not for sale.
pub fn value(world: &World, kind: BuildingKind) -> i64 {
    let c = &world.config.corps;
    match kind {
        BuildingKind::Bar => c.found_cost.bar,
        BuildingKind::Home => c.found_cost.home,
        BuildingKind::Farm => c.value.farm,
        BuildingKind::Market => c.value.market,
        BuildingKind::SecurityOffice => c.value.security_office,
        BuildingKind::Clinic => c.value.clinic,
        BuildingKind::Garage => c.value.garage,
        BuildingKind::Lab => c.value.lab,
        BuildingKind::Feed => c.value.feed,
        BuildingKind::Club => c.value.club,
        BuildingKind::Arcade => c.value.arcade,
        BuildingKind::NoodleBar => c.value.noodle_bar,
        BuildingKind::FightPit => c.value.fight_pit,
        BuildingKind::Den => c.value.den,
        BuildingKind::Lounge => c.value.lounge,
        BuildingKind::Fab => c.value.fab,
        BuildingKind::Fixer => c.value.fixer,
        // Real economy E26, E37.
        BuildingKind::Mission => c.value.mission,
        BuildingKind::Camp => c.value.camp,
        // Jobs & lives phase 1 (D4): the new kinds' `[buildings.<kind>] value`.
        k if BuildingKind::JOBS2.contains(&k) => world.config.buildings.for_kind(k).value.unwrap_or(0),
        _ => 0,
    }
}

/// D17: the role an owned niche building employs.
pub fn role_for(kind: BuildingKind) -> Option<Role> {
    match kind {
        BuildingKind::Farm => Some(Role::Farmer),
        BuildingKind::Market => Some(Role::Clerk),
        BuildingKind::Bar => Some(Role::Bartender),
        BuildingKind::SecurityOffice => Some(Role::Guard),
        BuildingKind::Clinic => Some(Role::Ripperdoc),
        BuildingKind::Garage => Some(Role::Mechanic),
        BuildingKind::Lab => Some(Role::Researcher),
        BuildingKind::Feed => Some(Role::Reporter),
        // L2 L2: one role per kind.
        BuildingKind::Club => Some(Role::Host),
        BuildingKind::Arcade => Some(Role::Attendant),
        BuildingKind::NoodleBar => Some(Role::Cook),
        BuildingKind::FightPit => Some(Role::Fighter),
        BuildingKind::Den => Some(Role::Croupier),
        BuildingKind::Lounge => Some(Role::Concierge),
        BuildingKind::Fab => Some(Role::Fabber),
        // M16a (plan C8).
        BuildingKind::Fixer => Some(Role::Fixer),
        // Real economy E26 (a Camp has no staff role, E37).
        BuildingKind::Mission => Some(Role::Volunteer),
        _ => None,
    }
}

/// Jobs and room J10: every role a `kind` employs with its places per floor:
/// the bespoke role first ([`role_for`], `corp_brain::full_staff`), then
/// each configured trade whose `workplace` lists the kind (its
/// `staff_per_floor`), in row order. Empty for a kind that employs nobody.
/// A `per_owned` trade's places are per owner group, not per floor
/// (`jobs::places_of` reads them).
pub fn roles_for(world: &World, kind: BuildingKind) -> Vec<(Role, usize)> {
    let mut out = Vec::new();
    if let Some(r) = role_for(kind) {
        out.push((r, crate::systems::corp_brain::full_staff(world, kind)));
    }
    if world.config.trades.is_empty() {
        return out;
    }
    // Class pay: a `classpay` row posts nothing with class pay off.
    let cfg = &world.config;
    // Jobs & lives phase 5: a `chip` row's posts are the contract's
    // (`dollhouse::daily`), never posted as vacancies.
    out.extend(cfg.trades_at(kind).iter().copied().filter(|&(r, _)| {
        r.trade()
            .and_then(|t| cfg.trade(t))
            .is_none_or(|row| crate::systems::classpay::trade_live(world, row) && !row.chip)
    }));
    out
}

/// Everyone employed at one of `buildings` (ascending, as `Corp.buildings`),
/// by workplace, each list ascending; a building with no staff has no
/// entry. Jobs and room J3: read from the employer index
/// (`World::employers`), not a scan of every worker per call.
pub fn staff_by_building(world: &World, buildings: &[EntityId]) -> BTreeMap<EntityId, Vec<EntityId>> {
    buildings
        .iter()
        .filter_map(|&b| world.employers.get(&b).filter(|v| !v.is_empty()).map(|v| (b, v.clone())))
        .collect()
}

/// Everyone employed at `building`, ascending (J3: the employer index).
pub fn staff_at(world: &World, building: EntityId) -> Vec<EntityId> {
    world.staff_of(building).to_vec()
}

/// Everyone employed at one of a corp's buildings (its exec too, if they
/// hold a job there), ascending.
pub fn employees_of(world: &World, corp: EntityId) -> Vec<EntityId> {
    let Some(c) = world.comp::<Corp>(corp) else { return Vec::new() };
    let mut out: Vec<EntityId> = staff_by_building(world, &c.buildings).into_values().flatten().collect();
    out.sort_unstable();
    out
}

/// D30: a building changes hands. Employees and open vacancies stay with the
/// building; a Home's rent is re-set from the new owner at the next pass.
pub fn transfer_building(world: &mut World, b: EntityId, to: Option<EntityId>) {
    let Some(from) = world.comp::<Building>(b).map(|bd| bd.owner) else { return };
    if from == to {
        return;
    }
    if let Some(c) = from.and_then(|f| world.comp_mut::<Corp>(f)) {
        if let Ok(i) = c.buildings.binary_search(&b) {
            c.buildings.remove(i);
        }
    }
    if let Some(c) = to.and_then(|t| world.comp_mut::<Corp>(t)) {
        if let Err(i) = c.buildings.binary_search(&b) {
            c.buildings.insert(i, b);
        }
    }
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.owner = to;
    }
    // M14 V4: the plane relinks (a flag; nothing reads it with the plane off).
    crate::systems::virt::mark_dirty(world);
}

/// A new corp entity (tests and incorporation).
pub fn spawn_corp(
    world: &mut World,
    name: String,
    niches: BTreeSet<Niche>,
    treasury: i64,
    exec: Option<EntityId>,
) -> EntityId {
    let id = world.spawn();
    world.insert(id, Corp::new(name, niches, treasury, exec));
    // M14 V21: a new corp's tree is [1, 1, 1] with the focus of its first
    // niche (the seeded corps are re-seeded by name in `World::new`).
    let tech = crate::systems::tech::seeded_tech(world, id);
    if let Some(c) = world.comp_mut::<Corp>(id) {
        c.tech = crate::virt::Tech::seeded([1, 1, 1], tech.focus);
    }
    crate::systems::virt::mark_dirty(world);
    id
}

/// Queue a shock for the corp brain; once the pending severities reach
/// `[corps] shock_severity_rethink`, `corp_brain::run` rescores this tick.
pub fn push_corp_shock(world: &mut World, corp: EntityId, shock: CorpShock) {
    let threshold = world.config.corps.shock_severity_rethink;
    let Some(c) = world.comp_mut::<Corp>(corp) else { return };
    if c.shocks.len() >= SHOCK_CAP {
        c.shocks.remove(0);
    }
    c.shocks.push(shock);
    if c.shocks.iter().map(|s| s.severity()).sum::<f32>() >= threshold {
        world.corp_rethink = true;
    }
}

/// D20: a crime cost a corp-owned building `coins`; `culprit` is the actor.
pub fn note_loss(world: &mut World, building: EntityId, coins: i64, culprit: Option<EntityId>) {
    let Some(corp) = world.corp_of_building(building) else { return };
    let gang = culprit.and_then(|a| world.gang_of(a));
    let tick = world.tick;
    let extortion = world.comp::<Building>(building).is_some_and(|b| b.kind == BuildingKind::Home);
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.loss_log.push_back(CorpLoss { tick, coins, gang, building: Some(building) });
    }
    let shock = if extortion { CorpShock::Extorted } else { CorpShock::Robbed(coins) };
    push_corp_shock(world, corp, shock);
}

/// D45: an agent owner died or left; each building goes to the spouse, else
/// the first living adult child, else the city.
pub fn on_owner_gone(world: &mut World, agent: EntityId) {
    let owned: Vec<EntityId> = world
        .with::<Building>()
        .into_iter()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.owner == Some(agent)))
        .collect();
    if owned.is_empty() {
        return;
    }
    let alive = |w: &World, id: EntityId| w.has::<Wallet>(id) && w.has::<Brain>(id) && !w.has::<Corpse>(id);
    let heir = world.spouse_of(agent).filter(|&s| s != agent && alive(world, s)).or_else(|| {
        crate::systems::demography::children_of_agent(world, agent)
            .into_iter()
            .filter(|&c| c != agent && alive(world, c) && crate::systems::demography::is_adult(world, c))
            .min()
    });
    let from = world.name_of(agent);
    for b in owned {
        transfer_building(world, b, heir);
        let what = world.name_of(b);
        let (actors, text) = match heir {
            Some(h) => (vec![h, b], format!("{what} inherited by {} from {from}", world.name_of(h))),
            None => (vec![b], format!("{what} passed to the city from {from}")),
        };
        world.push_event(EventKind::Acquired, &actors, text);
    }
}

/// The Nationalise lever: the Treasury buys a building at its value. A
/// derelict (M12 D26) has nobody to pay: the City repairs and re-lets it.
pub fn nationalise(world: &mut World, b: EntityId) -> Result<String, String> {
    let Some(bd) = world.comp::<Building>(b) else { return Err("no such building".into()) };
    let (kind, owner) = (bd.kind, bd.owner);
    if bd.derelict {
        let what = world.name_of(b);
        crate::systems::street::restore(world, b, None, "nationalised");
        return Ok(format!("the city took derelict {what} back"));
    }
    if owner.is_none() {
        return Err("already the city's".into());
    }
    let price = value(world, kind);
    if price <= 0 {
        return Err(format!("a {} is not for sale", kind.label()));
    }
    let coins = world.purse(None);
    if coins < price {
        return Err(format!("Treasury has {coins}, needs {price}"));
    }
    let seller = world.owner_label(owner);
    pay(world, None, owner, price, Flow::Sale);
    // As any sale: niches, contracts and staff follow (a nationalised
    // Security Office lets its guards go).
    crate::systems::corps::move_building(world, b, None);
    let what = world.name_of(b);
    let text = format!("the city nationalised {what} from {seller} for {price}");
    world.push_event(EventKind::Acquired, &[b], text.clone());
    Ok(text)
}

/// The Subsidise lever: Treasury -> a corp.
pub fn subsidise(world: &mut World, corp: EntityId, amount: i64) -> Result<String, String> {
    if !world.has::<Corp>(corp) {
        return Err("not a corp".into());
    }
    if amount <= 0 {
        return Err("bad amount".into());
    }
    let coins = world.purse(None);
    if coins < amount {
        return Err(format!("Treasury has {coins}, needs {amount}"));
    }
    charge(world, None, Some(corp), amount, Flow::Subsidy);
    Ok(format!("subsidised {} with {amount}", world.owner_label(Some(corp))))
}

// ---------------------------------------------------------------------------
// Seeding (the plan's Seeding table, D10-D12)
// ---------------------------------------------------------------------------

pub(super) fn door(world: &World, b: EntityId) -> crate::components::TilePos {
    world.comp::<Building>(b).map_or_else(Default::default, |bd| bd.door)
}

/// Deal the opening ownership from `[corps]`. No RNG: the world stream is
/// untouched. Missing buildings are skipped (a row asking for more than exist
/// takes what is left).
pub fn seed(world: &mut World) {
    // Every Hideout belongs to its gang.
    for g in world.gangs() {
        if let Some(h) = world.hideout_of(g) {
            if let Some(b) = world.comp_mut::<Building>(h) {
                b.owner = Some(g);
            }
        }
    }
    let cfg = world.config.corps.clone();
    let n = cfg.row_count();
    let mut corps = Vec::with_capacity(n);
    for i in 0..n {
        let niches = cfg.niches_of(i);
        let treasury = cfg.treasury_initial[i];
        let id = spawn_corp(world, cfg.names[i].clone(), niches, treasury, None);
        if let Some(c) = world.comp_mut::<Corp>(id) {
            c.slot = Some(u16::try_from(i).expect("[corps] at most 65,536 rows"));
            c.treasury_ref = treasury.max(1);
            if cfg.megacorp.get(i).copied().unwrap_or(false) {
                c.parent = Some(crate::components::OUTSIDE_PARENT_BASE + i as u32);
                c.outside_treasury = cfg.outside_treasury_initial;
            }
        }
        corps.push(id);
    }
    // 1. Farms, config order, file order.
    let mut farms = world.buildings_of_kind(BuildingKind::Farm).to_vec().into_iter();
    for (i, &c) in corps.iter().enumerate() {
        for _ in 0..cfg.farms[i] {
            let Some(f) = farms.next() else { break };
            transfer_building(world, f, Some(c));
        }
    }
    // 2. Markets: by farms descending (ties config order), the unassigned
    //    Market nearest the corp's first Farm (ties lower id).
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(cfg.farms[i]), i));
    let mut free_markets: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Market).to_vec();
    for &i in &order {
        let c = corps[i];
        let first_farm = world.comp::<Corp>(c).and_then(|cc| {
            cc.buildings
                .iter()
                .copied()
                .find(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Farm))
        });
        for _ in 0..cfg.markets[i] {
            let pick = match first_farm {
                Some(f) => {
                    let from = door(world, f);
                    free_markets.iter().copied().min_by_key(|&m| (door(world, m).manhattan(from), m))
                }
                None => free_markets.first().copied(),
            };
            let Some(m) = pick else { break };
            free_markets.retain(|&x| x != m);
            transfer_building(world, m, Some(c));
        }
    }
    // 3. Blocks: one pool by (tier descending, file order); corps by blocks
    //    descending (ties config order) take the next `blocks[i]`.
    let mut pool: Vec<(std::cmp::Reverse<u8>, usize, EntityId)> = world
        .buildings_of_kind(BuildingKind::Home)
        .iter()
        .enumerate()
        // M12 D26: the seeded derelicts belong to nobody.
        .filter(|&(_, &h)| world.comp::<Building>(h).is_some_and(|b| !b.derelict))
        .filter_map(|(k, &h)| world.comp::<Building>(h).map(|b| (std::cmp::Reverse(b.tier), k, h)))
        .collect();
    pool.sort();
    let mut pool = pool.into_iter().map(|(_, _, h)| h);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(cfg.blocks[i]), i));
    for &i in &order {
        for _ in 0..cfg.blocks[i] {
            let Some(h) = pool.next() else { break };
            transfer_building(world, h, Some(corps[i]));
        }
    }
    // 4. Security Offices, config order, file order; each posts its guards.
    let mut offices = world.buildings_of_kind(BuildingKind::SecurityOffice).to_vec().into_iter();
    for (i, &c) in corps.iter().enumerate() {
        for _ in 0..cfg.offices[i] {
            let Some(o) = offices.next() else { break };
            transfer_building(world, o, Some(c));
            let roles = world.vacancies.entry(o).or_default();
            roles.extend(std::iter::repeat_n(Role::Guard, cfg.security_guards as usize));
        }
    }
    // 5. Bars: the Food corp with the most Farms takes the Bar nearest its
    //    Market; then the first agent owners (D11).
    //    Scale S4: once per copy of the roster (`CorpsCfg::replicate`;
    //    copy `j` is the rows that are the `j`-th of their template).
    let mut free_bars: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Bar).to_vec();
    let copy_of = |i: usize| (0..i).filter(|&j| cfg.template_of(j) == cfg.template_of(i)).count();
    let copies = (0..n).map(copy_of).max().map_or(0, |c| c + 1);
    let food_leads: Vec<usize> = (0..copies)
        .filter_map(|copy| {
            (0..n)
                .filter(|&i| copy_of(i) == copy)
                .filter(|&i| world.comp::<Corp>(corps[i]).is_some_and(|c| c.niches.contains(&Niche::Food)))
                .min_by_key(|&i| (std::cmp::Reverse(cfg.farms[i]), i))
        })
        .collect();
    for i in food_leads {
        let market = world.comp::<Corp>(corps[i]).and_then(|cc| {
            cc.buildings
                .iter()
                .copied()
                .find(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Market))
        });
        if let Some(m) = market {
            let from = door(world, m);
            if let Some(bar) = free_bars.iter().copied().min_by_key(|&b| (door(world, b).manhattan(from), b)) {
                free_bars.retain(|&x| x != bar);
                transfer_building(world, bar, Some(corps[i]));
            }
        }
    }
    let mut owners: BTreeSet<EntityId> = BTreeSet::new();
    for bar in free_bars.into_iter().take(cfg.bar_owner_count) {
        let from = door(world, bar);
        let pick = world
            .citizens()
            .into_iter()
            .filter(|&a| world.has::<Brain>(a) && !world.has::<Job>(a) && !owners.contains(&a))
            .filter(|&a| crate::systems::demography::is_adult(world, a))
            .filter_map(|a| {
                let home = world.comp::<Household>(a).and_then(|h| h.home)?;
                Some((door(world, home).manhattan(from), a))
            })
            .min();
        let Some((_, owner)) = pick else { break };
        owners.insert(owner);
        transfer_building(world, bar, Some(owner));
        if let Some(w) = world.comp_mut::<Wallet>(owner) {
            w.coins = cfg.bar_owner_coins;
        }
    }
    // 6. Execs (D12): the greediest jobless adult per corp, config order.
    let mut execs: BTreeSet<EntityId> = BTreeSet::new();
    for &c in &corps {
        // L1: by age (`exec_min_age_years`), persuasion + knowledge, then
        // coins (the greediest jobless adult over 18 made a 19-year-old on
        // the dole Zetatech's exec); the M11 pick when nobody qualifies.
        let seasoned = crate::systems::life::pick_exec(
            world,
            world
                .citizens()
                .into_iter()
                .filter(|&a| world.has::<Brain>(a) && !world.has::<Job>(a))
                .filter(|&a| !execs.contains(&a) && !owners.contains(&a)),
        );
        let pick = seasoned.or_else(|| {
            world
                .citizens()
                .into_iter()
                .filter(|&a| world.has::<Brain>(a) && !world.has::<Job>(a))
                .filter(|&a| !execs.contains(&a) && !owners.contains(&a))
                .filter(|&a| crate::systems::demography::is_adult(world, a))
                .filter_map(|a| world.comp::<Personality>(a).map(|p| (p.greed, a)))
                .max_by(|x, y| x.0.total_cmp(&y.0).then(y.1.cmp(&x.1)))
                .map(|(_, a)| a)
        });
        if let Some(cc) = world.comp_mut::<Corp>(c) {
            cc.exec = pick;
        }
        if let Some(a) = pick {
            execs.insert(a);
            // L1: the exec lives in the Spire when a Block there has room.
            // The perf pass phase 10: after `Corp.exec` is set, so the move
            // reads his Hq (`life::exec_hq_door`; the base pick reads neither).
            crate::systems::life::house_exec(world, a);
        }
    }
    // Opening rents: corp Blocks at level 1.0, city Blocks the lever.
    for h in world.buildings_of_kind(BuildingKind::Home).to_vec() {
        let r = rent_for(world, h);
        if let Some(b) = world.comp_mut::<Building>(h) {
            b.rent_per_day = r;
        }
    }
}
