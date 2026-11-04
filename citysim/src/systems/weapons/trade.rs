use super::*;

/// The pressure pass phase 2 (OPEN-176; `[customers]`): a gang whose
/// armoury is under its target and whose purse holds a dealer's price
/// sends a member (a body first, else its lowest id; free, no dealer) to
/// buy from another gang's dealer with a surplus, before it ships abroad:
/// the quartermaster wants a weapon (the Shop goal walks him to the Bar),
/// the gang's purse pays, the weapon goes to its Hideout. One a gang a day;
/// a booking unspent by the next midnight is dropped.
pub(super) fn quartermasters(world: &mut World) {
    world.jobs2.arms.quarter.clear();
    if crate::systems::customers::cfg(world).is_none() {
        return;
    }
    let price = match dealer_price(world, 1) {
        Some(p) => p,
        None => return,
    };
    let sellers: BTreeSet<EntityId> =
        deal_bars(world, false).into_iter().filter_map(|b| bar_arms_dealer(world, b, false).map(|(_, g)| g)).collect();
    if sellers.is_empty() {
        return;
    }
    for g in world.gangs() {
        let Some(h) = world.hideout_of(g) else { continue };
        if stock_at(world, h).len() >= armoury_target(world, g) || !sellers.iter().any(|&x| x != g) {
            continue;
        }
        if world.purse(Some(g)) < price + world.config.weapons.coin_reserve {
            continue;
        }
        let Some(members) = world.comp::<Gang>(g).map(|x| x.members.clone()) else { continue };
        let fit = |w: &World, m: EntityId| {
            free_adult(w, m) && !crate::systems::stims::is_dealer(w, m) && !w.jobs2.arms.wants.contains(&m)
        };
        let body =
            |w: &World, m: EntityId| w.comp::<Brain>(m).is_some_and(|b| b.lod != crate::components::Lod::Statistical);
        let pick = members
            .iter()
            .copied()
            .find(|&m| fit(world, m) && body(world, m))
            .or_else(|| members.iter().copied().find(|&m| fit(world, m)));
        if let Some(m) = pick {
            world.jobs2.arms.quarter.insert(m, g);
            world.jobs2.arms.wants.insert(m);
        }
    }
}

/// The pressure pass round 2, R6b (OPEN-217; `[verbs]`): the gangs' gun
/// trade, hideout to hideout. On its trade day (`trade_days`, a hash of
/// the gang: no saved state) a gang under its armoury target whose purse
/// holds a weapon's price above `trade_reserve_days` of its members'
/// stipends buys up to `trade_lot` from the other gang with the largest
/// surplus (its Hideout nearest, then the lower id) at `trade_price_share`
/// of the dealer's T1 price a weapon: the seller's dealer (a registered
/// one, else its leader) makes the deal with the buyer's leader, the gang
/// purses pay (`Flow::Arms`), the weapons move armoury to armoury. The
/// quartermaster's walk-in (phase 2) buys one at the full price; this is
/// the lot a purse affords.
pub(super) fn gang_arms_trade(world: &mut World) {
    let Some(cfg) = world.config.verbs.clone() else { return };
    if cfg.trade_lot == 0 || cfg.trade_days == 0 {
        return;
    }
    let Some(list) = dealer_price(world, 1) else { return };
    let unit = ((list as f32 * cfg.trade_price_share).round() as i64).max(1);
    let day = world.day();
    let stipend = world.config.social.gang_stipend.max(0);
    for g in world.gangs() {
        // The gang's trade day: the hash's slot in the cycle.
        let slot = (hash(world, PURPOSE_TRADE, key(g), 0) * f32::from(cfg.trade_days)) as u64;
        if day % u64::from(cfg.trade_days) != slot.min(u64::from(cfg.trade_days) - 1) {
            continue;
        }
        let Some(h) = world.hideout_of(g) else { continue };
        let members = world.comp::<Gang>(g).map_or(0, |x| x.members.len());
        if members == 0 {
            continue;
        }
        let want = armoury_target(world, g).saturating_sub(stock_at(world, h).len());
        if want == 0 {
            continue;
        }
        let reserve = stipend * members as i64 * i64::from(cfg.trade_reserve_days);
        let budget = world.purse(Some(g)) - reserve;
        let afford = (budget.max(0) / unit) as usize;
        let n = want.min(usize::from(cfg.trade_lot)).min(afford);
        if n == 0 {
            continue;
        }
        let from = world.comp::<Building>(h).map(|b| b.door).unwrap_or_default();
        let seller = world
            .gangs()
            .into_iter()
            .filter(|&s| s != g)
            .filter_map(|s| {
                let left = surplus(world, s).len();
                let door = world.hideout_of(s).and_then(|x| world.comp::<Building>(x)).map(|b| b.door)?;
                (left > 0).then(|| (std::cmp::Reverse(left), door.manhattan(from), s))
            })
            .min()
            .map(|(_, _, s)| s);
        let Some(s) = seller else { continue };
        let lot: Vec<EntityId> = surplus(world, s).into_iter().take(n).collect();
        let price = unit * lot.len() as i64;
        if lot.is_empty() || ownership::pay(world, Some(g), Some(s), price, Flow::Arms) < price {
            continue;
        }
        let dealer = world
            .dealers
            .values()
            .flatten()
            .copied()
            .find(|&d| world.gang_of(d) == Some(s) && crate::systems::law::living(world, d))
            .or_else(|| world.comp::<Gang>(s).and_then(|x| x.leader));
        let buyer = world.comp::<Gang>(g).and_then(|x| x.leader);
        for &a in &lot {
            let p = prov(world, ProvKind::Dealt, dealer.or(Some(s)));
            hand_over(world, a, Some(g), AssetLoc::Stock(h), p, false);
        }
        ownership::jobs2_row(world).gang_arms_buys += 1;
        let (gn, sn) = (world.owner_label(Some(g)), world.owner_label(Some(s)));
        let who = |w: &World, e: Option<EntityId>| e.map_or_else(|| "nobody".to_string(), |x| w.name_of(x));
        let what = if lot.len() == 1 { "a weapon".to_string() } else { format!("{} weapons", lot.len()) };
        let text = format!(
            "{sn} sold {gn} {what} for {price} ({unit} each): {} dealt with {}; the lot moved hideout to hideout",
            who(world, dealer),
            who(world, buyer)
        );
        let mut actors = vec![g, s];
        actors.extend(dealer);
        actors.extend(buyer);
        world.push_event(EventKind::AssetBought, &actors, text);
    }
}

/// Each weapon an agent owns: carried today when the owner has a reason
/// (a gang member, a licence for a guard or Security post, the reasons to
/// want one), else on `carry_p_by_class`; else kept at home.
pub(super) fn carry_roll(world: &mut World) {
    let day = world.day();
    let items: Vec<(EntityId, EntityId, AssetLoc)> = world
        .jobs2
        .arms
        .index
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a).map(|x| (a, x)))
        .filter_map(|(a, x)| {
            let o = x.owner?;
            matches!(x.loc, AssetLoc::Carried(h) | AssetLoc::Parked(h) if h == o || world.comp::<Household>(o).and_then(|hh| hh.home) == Some(h))
                .then_some((a, o, x.loc))
        })
        .collect();
    for (a, o, loc) in items {
        if !free_adult(world, o) {
            continue;
        }
        let home = world.comp::<Household>(o).and_then(|h| h.home);
        let reason = world.gang_of(o).is_some() || licensed(world, o) || has_reason(world, o);
        let carry = reason
            || hash(world, PURPOSE_CARRY, key(o), day) < world.config.weapons.carry_p_by_class[class_index(world, o)];
        let want = match (carry, home) {
            (true, _) | (false, None) => AssetLoc::Carried(o),
            (false, Some(h)) => AssetLoc::Parked(h),
        };
        if want != loc {
            assets::set_loc(world, a, want);
        }
    }
}

/// The adults who want a weapon (no weapon owned, the coins for the
/// cheapest, and a reason or the class's stable share), and the licences
/// the lawful among them buy from the Civic Clerk while one is on the books.
pub(super) fn wants_and_licences(world: &mut World) {
    let cfg = world.config.weapons.clone();
    let cheapest = assets::list_price(world, AssetKind::Weapon, 1).unwrap_or(i64::MAX);
    let owners: BTreeSet<EntityId> =
        world.jobs2.arms.index.iter().filter_map(|&a| world.comp::<Asset>(a)).filter_map(|x| x.owner).collect();
    let mut wants = BTreeSet::new();
    let ids: Vec<EntityId> = world.with::<Identity>();
    for id in ids {
        if !free_adult(world, id) || owners.contains(&id) || world.gang_of(id).is_some() {
            continue;
        }
        if world.comp::<Job>(id).is_some_and(|j| j.role == crate::components::Role::Guard) {
            continue;
        }
        let coins = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
        if coins < cheapest + cfg.coin_reserve {
            continue;
        }
        let share = cfg.want_p_by_class[class_index(world, id)];
        if has_reason(world, id) || hash(world, PURPOSE_WANT, key(id), 0) < share {
            wants.insert(id);
        }
    }
    let fear = wants.iter().filter(|&&id| crate::systems::customers::frightened(world, id)).count() as u32;
    ownership::jobs2_row(world).fear_wants += fear;
    world.jobs2.arms.wants = wants;
    if crate::systems::industry::clerks_on_books(world) == 0 {
        return;
    }
    let buyers: Vec<EntityId> = world
        .jobs2
        .arms
        .wants
        .iter()
        .copied()
        .filter(|&id| !licensed(world, id))
        // Round 6 V3 (`OPEN-238`): not from a clinic's floor (V14
        // `ceo_91004 d25 00:07` bought a licence laid up at a Ripperdoc).
        .filter(|&id| !crate::systems::wounded::is_down(world, id))
        .filter(|&id| world.comp::<Personality>(id).is_some_and(|p| p.lawfulness >= 0.5))
        .filter(|&id| {
            world.comp::<Wallet>(id).is_some_and(|w| w.coins >= cfg.licence_fee + cheapest + cfg.coin_reserve)
        })
        .collect();
    for id in buyers {
        let paid = ownership::pay(world, Some(id), None, cfg.licence_fee, Flow::Licence);
        if paid < cfg.licence_fee {
            continue;
        }
        world.jobs2.arms.licences.insert(id);
        ownership::jobs2_row(world).licences_sold += 1;
        // Lives at work phase 5 (§ 2.2, civic clerk): the clerk named.
        let clerk = crate::systems::service::licence_clerk(world, id);
        let from = clerk.map(|(c, _)| format!(" from {}", world.name_of(c))).unwrap_or_default();
        let text = format!("{} bought a weapons licence for {}{from}", world.name_of(id), cfg.licence_fee);
        let actors: Vec<EntityId> = std::iter::once(id).chain(clerk.map(|(c, _)| c)).collect();
        world.push_event(EventKind::LicenceSold, &actors, text);
        if let Some((c, hall)) = clerk {
            crate::systems::service::licence_sold(world, c, hall, id, cfg.licence_fee);
        }
    }
}

// ---------------------------------------------------------------------------
// Seeding and the day's snapshot
// ---------------------------------------------------------------------------

/// `World::new`, after the industries: a Gunsmith per copy of each
/// `gunsmith_rows` corp, and the opening stocks (no RNG; no coins: the
/// seeded city's stock is unpaid, as its buildings are): the Precinct at
/// `stock_per_guard` per city guard, each gang's armoury at half its
/// target, each Gunsmith at half its cap (tier 1).
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    let cfg = world.config.weapons.clone();
    let copies = world.lot_copies();
    for j in 0..copies {
        for &r in &cfg.gunsmith_rows {
            let Some(t) = world.config.corps.row_template(r) else { continue };
            let Some(&c) = world.corps_of_template(t).get(j) else { continue };
            crate::systems::industry::seed_kind(world, BuildingKind::Gunsmith, Some(c));
        }
    }
    let now = world.tick;
    if let Some(p) = precinct(world) {
        let city = world.guards().iter().filter(|&&g| crate::systems::law::is_city_guard(world, g)).count();
        let n = (city as f32 * cfg.stock_per_guard).ceil() as usize;
        for _ in 0..n {
            let pv = Provenance { how: ProvKind::Seized, by: Some(p), at: now };
            spawn_weapon(world, cfg.guard_tier, None, AssetLoc::Stock(p), pv, false);
        }
    }
    for gang in world.gangs() {
        let Some(h) = world.hideout_of(gang) else { continue };
        let n = armoury_target(world, gang) / 2;
        for _ in 0..n {
            let pv = Provenance { how: ProvKind::Smuggled, by: Some(gang), at: now };
            spawn_weapon(world, 1, Some(gang), AssetLoc::Stock(h), pv, false);
        }
    }
    for b in world.buildings_of_kind(BuildingKind::Gunsmith).to_vec() {
        let owner = world.owner_of(b);
        for _ in 0..cfg.gunsmith_stock / 2 {
            let pv = Provenance { how: ProvKind::Made, by: Some(b), at: now };
            spawn_weapon(world, 1, owner, AssetLoc::Stock(b), pv, false);
        }
    }
    guard_shares(world);
}

/// The day's end (`systems::stats`): the snapshots of the weapons group.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let mut alive = 0u32;
    let mut carriers: BTreeSet<EntityId> = BTreeSet::new();
    let (mut armoury, mut precinct_n, mut shops) = (0u32, 0u32, 0u32);
    let pre = precinct(world);
    for &a in &world.jobs2.arms.index {
        let Some(x) = world.comp::<Asset>(a) else { continue };
        if x.kind != AssetKind::Weapon {
            continue;
        }
        alive += 1;
        match x.loc {
            AssetLoc::Carried(h) => {
                carriers.insert(h);
            }
            AssetLoc::Stock(b) => {
                match world.comp::<Building>(b).map(|bd| bd.kind) {
                    Some(BuildingKind::Hideout) => armoury += 1,
                    Some(BuildingKind::Gunsmith) => shops += 1,
                    _ => {}
                }
                if Some(b) == pre {
                    precinct_n += 1;
                }
            }
            _ => {}
        }
    }
    let licences = world.jobs2.arms.licences.len() as u32;
    let wants = world.jobs2.arms.wants.len() as u32;
    let row = ownership::jobs2_row(world);
    row.weapons = alive;
    row.carriers = carriers.len() as u32;
    row.armoury = armoury;
    row.precinct_arms = precinct_n;
    row.gunsmith_stock = shops;
    row.licences = licences;
    row.weapon_wants = wants;
}

/// God (`FloodGuns`): `n` weapons at `tier` into a gang's armoury, past the
/// store's cap (the god scenario's lever). Returns the weapons made.
pub fn flood(world: &mut World, gang: EntityId, n: u32, tier: u8) -> Result<u32, String> {
    if !on(world) {
        return Err("weapons are off".into());
    }
    let h = world.hideout_of(gang).ok_or("no such gang with a Hideout")?;
    let now = world.tick;
    let mut made = 0;
    for _ in 0..n {
        let pv = Provenance { how: ProvKind::Granted, by: None, at: now };
        if spawn_weapon(world, tier, Some(gang), AssetLoc::Stock(h), pv, true).is_some() {
            made += 1;
        }
    }
    Ok(made)
}
