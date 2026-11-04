use super::*;

/// `per_1000 × population ÷ 1,000`, at least 1.
pub(crate) fn per_capita_day(world: &World, per_1000: f32) -> u32 {
    ((f64::from(world.config.world.population) * f64::from(per_1000) / 1000.0).round() as u32).max(1)
}

// ---------------------------------------------------------------------------
// Buying: the Shop goal's weapon offer (§ 4)
// ---------------------------------------------------------------------------

/// A weapon's price at a Gunsmith: `list × the seller's price level`.
fn legal_price(world: &World, seller: EntityId, tier: u8) -> Option<i64> {
    let list = assets::list_price(world, AssetKind::Weapon, tier)?;
    Some((list as f32 * assets::seller_level(world, seller)).round().max(1.0) as i64)
}

/// A dealer's price: `list × dealer_markup`.
pub(super) fn dealer_price(world: &World, tier: u8) -> Option<i64> {
    let list = assets::list_price(world, AssetKind::Weapon, tier)?;
    Some((list as f32 * world.config.weapons.dealer_markup).round().max(1.0) as i64)
}

/// A Gunsmith sells while one of its gunsmiths is on shift (or, off
/// screen, on the books) and it has stock.
fn gunsmith_open(world: &World, b: EntityId) -> bool {
    if !crate::systems::industry::open(world, b) || world.is_closed(b) {
        return false;
    }
    let tod = world.tick_of_day();
    world.staff_of(b).iter().any(|&s| world.comp::<Job>(s).is_some_and(|j| j.on_shift(tod)))
}

/// A gang's armoury target: `armoury_per_member × members` (rounded up;
/// the muster's stock, kept back from the dealer).
pub(super) fn armoury_target(world: &World, gang: EntityId) -> usize {
    let n = world.comp::<Gang>(gang).map_or(0, |g| g.members.len()) as f32;
    (n * world.config.weapons.armoury_per_member).ceil() as usize
}

/// Is there an armoury surplus for `gang`'s dealer to sell (the shadow
/// tool's arms-dealer pick)?
pub fn has_surplus(world: &World, gang: EntityId) -> bool {
    !surplus(world, gang).is_empty()
}

/// The armoury's weapons beyond its target (the dealer's to sell), the
/// cheapest first.
pub(super) fn surplus(world: &World, gang: EntityId) -> Vec<EntityId> {
    let Some(h) = world.hideout_of(gang) else { return Vec::new() };
    let mut stock = stock_at(world, h);
    let keep = armoury_target(world, gang);
    if stock.len() <= keep {
        return Vec::new();
    }
    // The best stay for the members; the rest are for sale, cheapest first.
    stock.sort_by_key(|&a| {
        let t = world.comp::<Asset>(a).map_or(0, |x| x.tier);
        (t, a)
    });
    let n = stock.len() - keep;
    stock.truncate(n);
    stock
}

/// The dealer selling at `bar` whose gang has a surplus, with that gang:
/// a registered dealer alive and in it (`present`, a body walking in), else
/// (off screen, the Statistical shop) the bar's last dealer, alive and free.
pub(super) fn bar_arms_dealer(world: &World, bar: EntityId, present: bool) -> Option<(EntityId, EntityId)> {
    let here = world.dealers.get(&bar).and_then(|list| {
        list.iter().copied().find_map(|d| {
            let in_it = crate::systems::law::living(world, d)
                && world.comp::<Position>(d).is_some_and(|p| p.building == Some(bar));
            let gang = world.gang_of(d)?;
            (in_it && !surplus(world, gang).is_empty()).then_some((d, gang))
        })
    });
    if here.is_some() || present {
        return here;
    }
    let &(d, _) = world.deal_log.get(&bar)?;
    let gang = world.gang_of(d)?;
    (crate::systems::law::living(world, d) && !world.has::<Sentence>(d) && !surplus(world, gang).is_empty())
        .then_some((d, gang))
}

/// The Bars a dealer sells at (registered now, or logged).
pub(super) fn deal_bars(world: &World, present: bool) -> Vec<EntityId> {
    let mut v: Vec<EntityId> = world.dealers.keys().copied().collect();
    if !present {
        v.extend(world.deal_log.keys().copied());
        v.sort_unstable();
        v.dedup();
    }
    v
}

/// Does `id` want a weapon of its own (the midnight pass's set)?
pub fn wants(world: &World, id: EntityId) -> bool {
    on(world) && world.jobs2.arms.wants.contains(&id)
}

/// The Shop goal's weapon offer for `id`: `(seller, tier, price, legal)`:
/// the dearest affordable at the nearest Gunsmith with stock (open now
/// when `require_open`) for a licence holder, else the cheapest at the
/// nearest dealer's Bar. `None` without the want, the coins, or a seller.
pub fn offer(
    world: &World,
    id: EntityId,
    from: crate::components::TilePos,
    require_open: bool,
) -> Option<(EntityId, u8, i64, bool)> {
    // The pressure pass phase 2: a quartermaster buys for his gang's
    // armoury with its purse, from another gang's dealer only.
    let qm = world.jobs2.arms.quarter.get(&id).copied();
    if !wants(world, id) || (qm.is_none() && world.comp::<Kit>(id).is_some_and(|k| k.weapon > 0)) {
        return None;
    }
    let _t = crate::prof::scope!("weapons::offer");
    let coins = match qm {
        Some(g) => world.purse(Some(g)) - world.config.weapons.coin_reserve,
        None => world.comp::<Wallet>(id).map_or(0, |w| w.coins) - world.config.weapons.coin_reserve,
    };
    if coins <= 0 {
        return None;
    }
    let door = |b: EntityId| world.comp::<Building>(b).map_or(u32::MAX, |bd| bd.door.manhattan(from));
    if qm.is_none() && licensed(world, id) {
        let shop = world
            .buildings_of_kind(BuildingKind::Gunsmith)
            .iter()
            .copied()
            .filter(|&b| crate::systems::industry::open(world, b) && (!require_open || gunsmith_open(world, b)))
            .filter(|&b| !stock_at(world, b).is_empty())
            .min_by_key(|&b| (door(b), b));
        if let Some(b) = shop {
            let pick = stock_at(world, b)
                .into_iter()
                .filter_map(|a| world.comp::<Asset>(a).map(|x| x.tier))
                .filter_map(|t| legal_price(world, b, t).map(|p| (t, p)))
                .filter(|&(_, p)| p <= coins)
                .max_by_key(|&(t, p)| (p, t));
            if let Some((t, p)) = pick {
                return Some((b, t, p, true));
            }
        }
    }
    let other = |b: EntityId| bar_arms_dealer(world, b, require_open).is_some_and(|(_, g)| Some(g) != qm);
    let bar = deal_bars(world, require_open).into_iter().filter(|&b| other(b)).min_by_key(|&b| (door(b), b))?;
    let (_, gang) = bar_arms_dealer(world, bar, require_open)?;
    let t = surplus(world, gang).first().and_then(|&a| world.comp::<Asset>(a)).map(|x| x.tier)?;
    let p = dealer_price(world, t)?;
    (p <= coins).then_some((bar, t, p, false))
}

/// The pre-view pass phase 4 (P3, OPEN-119): an off-screen buyer's sale
/// from a dealer who is a body, booked at midnight and made when the
/// dealer's body is at the Bar (`World::arms_due`, saved).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmsMeet {
    pub buyer: EntityId,
    pub bar: EntityId,
    pub pick: ShopPick,
    /// The meeting lapses after this tick (`[presence] arms_wait_hours`).
    pub until: Tick,
}

/// The pre-view pass phase 4 (`assets::stat_shop`, midnight): a
/// Statistical buyer's weapon from a dealer's Bar. With `[presence]` on and
/// the Bar's dealer a body, the sale is booked for the meeting (`true`);
/// else (off, a Gunsmith, a Statistical dealer: the mirror) `false`, and
/// the caller sells off screen now (`arms_midnight` counts the dealer's).
pub fn book_meeting(world: &mut World, buyer: EntityId, seller: EntityId, pick: &ShopPick) -> bool {
    let Some(cfg) = world.config.presence.as_ref() else { return false };
    if world.comp::<Building>(seller).is_none_or(|b| b.kind == BuildingKind::Gunsmith) {
        return false;
    }
    let Some((dealer, _)) = bar_arms_dealer(world, seller, false) else { return false };
    if !crate::systems::presence::is_body(world, dealer) {
        ownership::jobs2_row(world).arms_midnight += 1;
        return false;
    }
    let until = world.tick + Tick::from(cfg.arms_wait_hours) * TICKS_PER_HOUR;
    if !world.arms_due.iter().any(|m| m.buyer == buyer) {
        world.arms_due.push(ArmsMeet { buyer, bar: seller, pick: pick.clone(), until });
    }
    true
}

/// The pre-view pass phase 4 (P3, OPEN-119), hourly: each booked arms sale
/// whose dealer's body stands in the Bar now is made there (`buy` with the
/// dealer present: the coins, the hand-over, `ArmsDealing` raised at his
/// tile, the witnesses the bodies present); lapsed meetings and buyers gone
/// are dropped (`arms_missed`), the rest wait.
pub fn arms_meetings(world: &mut World) {
    if world.arms_due.is_empty() || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let now = world.tick;
    let due = std::mem::take(&mut world.arms_due);
    let mut keep = Vec::with_capacity(due.len());
    for m in due {
        let gone = !crate::systems::law::living(world, m.buyer) || world.has::<Sentence>(m.buyer);
        if gone || now > m.until {
            ownership::jobs2_row(world).arms_missed += 1;
            continue;
        }
        if bar_arms_dealer(world, m.bar, true).is_none() {
            keep.push(m);
            continue;
        }
        if buy(world, m.buyer, m.bar, &m.pick, true).is_ok() {
            ownership::jobs2_row(world).arms_meet += 1;
        } else {
            ownership::jobs2_row(world).arms_missed += 1;
        }
    }
    keep.append(&mut world.arms_due);
    world.arms_due = keep;
}

/// May `BuyAsset` run for a weapon at `b` now (an open Gunsmith with
/// stock, or a Bar with an arms dealer)?
pub fn sale_open(world: &World, b: EntityId) -> bool {
    match world.comp::<Building>(b).map(|bd| bd.kind) {
        Some(BuildingKind::Gunsmith) => gunsmith_open(world, b) && !stock_at(world, b).is_empty(),
        Some(_) => bar_arms_dealer(world, b, true).is_some(),
        None => false,
    }
}

/// `BuyAsset` for a weapon at `seller`: from a Gunsmith's stock to a
/// licence holder (`Flow::Asset`, taxed), or from a dealer's armoury
/// surplus (`Flow::Arms` to the gang, the dealer's cut, `ArmsDealing`
/// raised against the dealer with the buyer no witness). `present`: a
/// body at the counter (the Statistical shop buys off screen).
pub fn buy(
    world: &mut World,
    buyer: EntityId,
    seller: EntityId,
    pick: &ShopPick,
    present: bool,
) -> Result<EntityId, String> {
    if !on(world) {
        return Err("weapons are off".into());
    }
    let coins = world.purse(Some(buyer));
    let kind = world.comp::<Building>(seller).map(|b| b.kind).ok_or("no such seller")?;
    let now = world.tick;
    if kind == BuildingKind::Gunsmith {
        if !licensed(world, buyer) {
            return Err("no licence".into());
        }
        let a = stock_at(world, seller)
            .into_iter()
            .find(|&a| world.comp::<Asset>(a).is_some_and(|x| x.tier == pick.tier))
            .ok_or("sold out")?;
        let price = legal_price(world, seller, pick.tier).ok_or("no price")?;
        if coins < price {
            return Err("cannot afford".into());
        }
        let owner = world.owner_of(seller);
        let paid = ownership::pay(world, Some(buyer), owner, price, Flow::Asset);
        ownership::credit(world, seller, paid);
        let p = prov(world, ProvKind::Sold, Some(seller));
        hand_over(world, a, Some(buyer), AssetLoc::Carried(buyer), p, false);
        ownership::jobs2_row(world).sold_legal += 1;
        if let Some(b) = world.comp_mut::<crate::components::Body>(buyer) {
            b.last_shop = Some(now);
        }
        let text =
            format!("{} bought a Weapon T{} at {} for {price}", world.name_of(buyer), pick.tier, world.name_of(seller));
        world.push_event(EventKind::AssetBought, &[buyer, seller, a], text);
        world.jobs2.arms.wants.remove(&buyer);
        return Ok(a);
    }
    let (dealer, gang) = bar_arms_dealer(world, seller, present).ok_or("no dealer")?;
    let qm = world.jobs2.arms.quarter.get(&buyer).copied();
    if qm == Some(gang) {
        return Err("his own gang's dealer".into());
    }
    let coins = qm.map_or(coins, |g| world.purse(Some(g)));
    let a = surplus(world, gang).into_iter().next().ok_or("sold out")?;
    let tier = world.comp::<Asset>(a).map_or(1, |x| x.tier);
    let price = dealer_price(world, tier).ok_or("no price")?;
    if coins < price {
        return Err("cannot afford".into());
    }
    ownership::pay(world, Some(qm.unwrap_or(buyer)), Some(gang), price, Flow::Arms);
    let cut = world.config.weapons.dealer_cut.min(price);
    ownership::pay(world, Some(gang), Some(dealer), cut, Flow::Arms);
    let p = prov(world, ProvKind::Dealt, Some(dealer));
    match qm.and_then(|g| world.hideout_of(g).map(|h| (g, h))) {
        Some((g, h)) => {
            hand_over(world, a, Some(g), AssetLoc::Stock(h), p, false);
            world.jobs2.arms.quarter.remove(&buyer);
            ownership::jobs2_row(world).gang_arms_buys += 1;
        }
        None => hand_over(world, a, Some(buyer), AssetLoc::Carried(buyer), p, false),
    }
    ownership::jobs2_row(world).sold_dealer += 1;
    if let Some(b) = world.comp_mut::<crate::components::Body>(buyer) {
        b.last_shop = Some(now);
    }
    let tile = world.comp::<Position>(dealer).map_or_else(Default::default, |p| p.tile);
    crate::systems::law::raise_crime_except(
        world,
        dealer,
        None,
        None,
        crate::components::Crime::ArmsDealing,
        tile,
        Some(buyer),
    );
    let text = match qm {
        Some(g) => format!(
            "{} bought a Weapon T{tier} from {} at {} for {price}, for {}'s armoury",
            world.name_of(buyer),
            world.name_of(dealer),
            world.name_of(seller),
            world.owner_label(Some(g))
        ),
        None => format!(
            "{} bought a Weapon T{tier} from {} at {} for {price}",
            world.name_of(buyer),
            world.name_of(dealer),
            world.name_of(seller)
        ),
    };
    world.push_event(EventKind::AssetBought, &[buyer, seller, a, dealer], text);
    // The pressure pass phase 2: the dealer's side of it (a body's diary).
    let sal = world.config.service.served_salience;
    world.remember(dealer, MemoryKind::Served, Some(buyer), sal, 0.1, false);
    world.remember(buyer, MemoryKind::Served, Some(dealer), sal, 0.1, false);
    world.jobs2.arms.wants.remove(&buyer);
    Ok(a)
}

// ---------------------------------------------------------------------------
// Gunsmiths (§ 4)
// ---------------------------------------------------------------------------

/// `trades::accrue`'s Gunsmith arm: a gunsmith's `ticks` on shift make
/// `make_rate × (0.5 + knowledge)` weapons an hour from `parts_per_weapon`
/// Parts each, at the owner corp's Industry tier (its maker), up to the
/// shop's stock cap and the store's.
pub fn accrue_gunsmith(world: &mut World, worker: EntityId, b: EntityId, ticks: u64) {
    if !on(world) || !crate::systems::industry::open(world, b) {
        return;
    }
    let cfg = world.config.weapons.clone();
    let knowledge = world.comp::<Skills>(worker).map_or(0.0, |s| s.knowledge);
    let units = cfg.make_rate * (0.5 + knowledge) * ticks as f32 / TICKS_PER_HOUR as f32;
    let whole = {
        let Some(bd) = world.comp_mut::<Building>(b) else { return };
        bd.production_accum += units;
        let whole = bd.production_accum.floor().max(0.0);
        bd.production_accum -= whole;
        whole as u32
    };
    let per = cfg.parts_per_weapon.max(1);
    for _ in 0..whole {
        if stock_at(world, b).len() >= cfg.gunsmith_stock as usize
            || world.stock(b, crate::components::Good::Parts) < per
        {
            break;
        }
        let owner = world.owner_of(b);
        let maker = assets::maker_for(world, b, AssetKind::Weapon);
        let t = maker
            .and_then(|m| world.comp::<crate::components::Corp>(m))
            .filter(|c| !c.tech.is_unset())
            .map_or(1, |c| c.tech.tier_of(crate::virt::Track::Industry))
            .clamp(1, 3);
        let p = prov(world, ProvKind::Made, Some(b));
        let Some(a) = spawn_weapon(world, t, owner, AssetLoc::Stock(b), p, false) else { break };
        world.take_stock(b, crate::components::Good::Parts, per);
        if let Some(x) = world.comp_mut::<Asset>(a) {
            x.maker = maker;
        }
        ownership::jobs2_row(world).weapons_made += 1;
    }
}

/// Sell a Gunsmith's weapon to a building's owner for its stock (the
/// Treasury for the Precinct, a corp for its Office): `Flow::Asset`.
/// Returns the coins paid.
fn sell_to_stock(world: &mut World, shop: EntityId, a: EntityId, buyer: Option<EntityId>, to: EntityId) -> i64 {
    let tier = world.comp::<Asset>(a).map_or(1, |x| x.tier);
    let Some(price) = legal_price(world, shop, tier) else { return 0 };
    let seller = world.owner_of(shop);
    let paid = if seller == buyer { 0 } else { ownership::charge(world, buyer, seller, price, Flow::Asset) };
    ownership::credit(world, shop, paid);
    let p = prov(world, ProvKind::Sold, Some(shop));
    hand_over(world, a, buyer, AssetLoc::Stock(to), p, false);
    ownership::jobs2_row(world).sold_legal += 1;
    paid.max(price * i64::from(seller == buyer))
}

/// The nearest Gunsmith to `from` with stock, with its cheapest weapon.
fn nearest_stock(world: &World, from: crate::components::TilePos) -> Option<(EntityId, EntityId)> {
    world
        .buildings_of_kind(BuildingKind::Gunsmith)
        .iter()
        .copied()
        .filter(|&b| crate::systems::industry::open(world, b))
        .filter_map(|b| {
            let mut s = stock_at(world, b);
            s.sort_by_key(|&a| (world.comp::<Asset>(a).map_or(0, |x| x.tier), a));
            let a = *s.first()?;
            world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b, a))
        })
        .min()
        .map(|(_, b, a)| (b, a))
}

/// Midnight: the Treasury's arms line buys for the Precinct up to its
/// target (`stock_per_guard` per city guard) within `arms_budget`, and the
/// seized surplus past the target is destroyed (the worst first); each
/// Security corp buys for its Offices (per private guard) while it holds
/// the coins. From the Gunsmiths' stock, the nearest first.
pub(super) fn restock_guards(world: &mut World) {
    let cfg = world.config.weapons.clone();
    if let Some(p) = precinct(world) {
        let city: usize = world.guards().iter().filter(|&&g| crate::systems::law::is_city_guard(world, g)).count();
        let target = (city as f32 * cfg.stock_per_guard).ceil() as usize;
        let mut budget = cfg.arms_budget;
        let door = world.comp::<Building>(p).map(|b| b.door).unwrap_or_default();
        while stock_at(world, p).len() < target && budget > 0 {
            let Some((shop, a)) = nearest_stock(world, door) else { break };
            let tier = world.comp::<Asset>(a).map_or(1, |x| x.tier);
            let Some(price) = legal_price(world, shop, tier) else { break };
            if price > budget || world.purse(None) < price {
                break;
            }
            budget -= sell_to_stock(world, shop, a, None, p);
        }
        // The seized surplus past the target is destroyed (the worst first).
        let mut s = stock_at(world, p);
        if s.len() > target {
            let over = s.len() - target;
            s.sort_by_key(|&a| (world.comp::<Asset>(a).map_or(0, |x| assets::eff_tier_of(world, x)), a));
            for a in s.into_iter().take(over) {
                assets::despawn(world, a);
                world.jobs2.arms.index.remove(&a);
            }
        }
    }
    let offices: Vec<EntityId> = world.buildings_of_kind(BuildingKind::SecurityOffice).to_vec();
    for o in offices {
        let Some(corp) = world.corp_of_building(o) else { continue };
        let guards =
            world.guards().iter().filter(|&&g| world.comp::<Job>(g).is_some_and(|j| j.employer == Some(o))).count();
        let target = (guards as f32 * cfg.stock_per_guard).ceil() as usize;
        let door = world.comp::<Building>(o).map(|b| b.door).unwrap_or_default();
        while stock_at(world, o).len() < target {
            let Some((shop, a)) = nearest_stock(world, door) else { break };
            let tier = world.comp::<Asset>(a).map_or(1, |x| x.tier);
            let Some(price) = legal_price(world, shop, tier) else { break };
            if world.purse(Some(corp)) < price * 2 {
                break;
            }
            sell_to_stock(world, shop, a, Some(corp), o);
        }
    }
}

/// The guards' armed shares: the stock over the headcount, and the best
/// tier in it.
pub(super) fn guard_shares(world: &mut World) {
    let tier_of = |w: &World, b: EntityId| -> (usize, u8) {
        let s = stock_at(w, b);
        let t = s.iter().filter_map(|&a| w.comp::<Asset>(a)).map(|x| assets::eff_tier_of(w, x).max(1)).max();
        (s.len(), t.unwrap_or(0))
    };
    let law = match precinct(world) {
        Some(p) => {
            let (n, t) = tier_of(world, p);
            let city = world.guards().iter().filter(|&&g| crate::systems::law::is_city_guard(world, g)).count();
            ((n as f32 / city.max(1) as f32).min(1.0), t)
        }
        None => (0.0, 0),
    };
    let mut offices = BTreeMap::new();
    for o in world.buildings_of_kind(BuildingKind::SecurityOffice).to_vec() {
        let (n, t) = tier_of(world, o);
        if n == 0 {
            continue;
        }
        let guards =
            world.guards().iter().filter(|&&g| world.comp::<Job>(g).is_some_and(|j| j.employer == Some(o))).count();
        offices.insert(o, ((n as f32 / guards.max(1) as f32).min(1.0), t));
    }
    world.jobs2.arms.law = law;
    world.jobs2.arms.offices = offices;
}

// ---------------------------------------------------------------------------
// Armouries and crates (§ 4)
// ---------------------------------------------------------------------------

/// `raid::depart`: a member marching out draws the armoury's best weapon
/// (the gang's still; returned at midnight).
pub fn issue(world: &mut World, member: EntityId) {
    if !on(world) || world.comp::<Kit>(member).is_some_and(|k| k.weapon > 0) {
        return;
    }
    let Some(gang) = world.gang_of(member) else { return };
    let Some(h) = world.hideout_of(gang) else { return };
    let Some(a) = best_of(world, &stock_at(world, h)) else { return };
    assets::set_loc(world, a, AssetLoc::Carried(member));
    ownership::jobs2_row(world).issued += 1;
}

/// Midnight: every gang weapon carried by a member goes back to the
/// armoury (a jailed or dead member's stays where the law or the street
/// left it).
pub(super) fn return_issued(world: &mut World) {
    let out: Vec<(EntityId, EntityId)> = world
        .jobs2
        .arms
        .index
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a).map(|x| (a, x)))
        .filter_map(|(a, x)| match (x.owner, x.loc) {
            (Some(g), AssetLoc::Carried(m)) if world.has::<Gang>(g) && m != g => Some((a, g)),
            _ => None,
        })
        .collect();
    for (a, g) in out {
        let Some(h) = world.hideout_of(g) else { continue };
        let holder = world.comp::<Asset>(a).and_then(|x| x.loc.holder());
        if holder.is_some_and(|m| world.has::<Sentence>(m) || world.has::<Corpse>(m)) {
            continue;
        }
        assets::set_loc(world, a, AssetLoc::Stock(h));
    }
}
