use super::*;

// ---------------------------------------------------------------------------
// Buying (plan D29, D30) and god grants
// ---------------------------------------------------------------------------

/// Where a new asset of `kind` goes for `buyer` at `seller`.
fn new_loc(world: &World, kind: AssetKind, buyer: EntityId, seller: EntityId) -> AssetLoc {
    let agent = world.has::<Identity>(buyer);
    match kind {
        k if k.is_vehicle() => AssetLoc::Parked(seller),
        AssetKind::Implant(_) if agent => AssetLoc::Installed(buyer),
        AssetKind::Pack | AssetKind::Bridge | AssetKind::Deck | AssetKind::Weapon if agent => AssetLoc::Carried(buyer),
        // M13 D42: a robot waits in the seller's stock until its buyer posts
        // it (`robots::consider_robot` posts it at the building Secure named).
        _ => AssetLoc::Stock(seller),
    }
}

/// `buyer` (an agent, gang or corp) buys `pick` at `seller` (plan D29):
/// price `round(list × level)` (a used asset `used_frac × list ×
/// condition / 100`), in full or `down_frac` down with a `Finance` to the
/// seller's owner; a new asset's import is paid by the seller's owner
/// first (refused: no sale). `Flow::Asset`, the `AssetBought` event, the
/// seller's credit and sales count, the buyer's `last_shop`.
pub fn buy(world: &mut World, buyer: EntityId, seller: EntityId, pick: &ShopPick) -> Result<EntityId, String> {
    buy_noted(world, buyer, seller, pick, None)
}

/// What a sale would cost, read without moving a coin ([`quote`]).
struct Quote {
    used: Option<(EntityId, i64, u8, AssetKind, u8)>,
    kind: AssetKind,
    tier: u8,
    list: i64,
    price: i64,
    financed: bool,
    down: i64,
    /// A new asset's parts taken from the seller's stock and its import.
    parts: u32,
    import: i64,
    /// The seller's owner, a Corp under the day's import cap, fronts `import`.
    capped: bool,
}

/// The refusals of [`buy_noted`], in its order, read only: the sale
/// `buyer` would make for `pick` at `seller`, or why there is none.
fn quote(world: &World, buyer: EntityId, seller: EntityId, pick: &ShopPick) -> Result<Quote, String> {
    let cfg = &world.config.assets;
    if !world.has::<Building>(seller) {
        return Err("no such seller".into());
    }
    let seller_owner = world.owner_of(seller);
    let used = match pick.used {
        Some(u) => {
            let x = world.comp::<Asset>(u).ok_or("the used asset is gone")?;
            if x.loc != AssetLoc::Stock(seller) {
                return Err("the used asset is not in this seller's stock".into());
            }
            Some((u, x.list, x.condition, x.kind, x.tier))
        }
        None => None,
    };
    let (kind, tier) = used.map_or((pick.kind, pick.tier), |(_, _, _, k, t)| (k, t));
    let list = match used {
        Some((_, l, _, _, _)) => l,
        None => list_price(world, kind, tier).ok_or_else(|| format!("no {} T{tier} for sale", kind.label()))?,
    };
    // M14 V22: new stock only while the seller's tech allows it.
    if used.is_none() && !can_sell(world, seller, kind, tier) {
        return Err(format!("{} cannot sell a {} T{tier} (tech)", world.name_of(seller), kind.label()));
    }
    let price = match used {
        Some((_, l, c, _, _)) => (world.config.chrome.used_frac * l as f32 * f32::from(c) / 100.0).round() as i64,
        None => (list as f32 * seller_level(world, seller)).round() as i64,
    };
    let coins = world.purse(Some(buyer));
    let financed = coins < price;
    let down = if financed { (cfg.down_frac_of(kind) * price as f32).round() as i64 } else { price };
    if coins < down || matches!(ownership::owner_kind(world, Some(buyer)), OwnerKind::City) {
        return Err(format!("{} cannot afford {price}", world.owner_label(Some(buyer))));
    }
    let mut q = Quote { used, kind, tier, list, price, financed, down, parts: 0, import: 0, capped: false };
    // D30: the import, new assets only, paid first by the seller's owner.
    if used.is_none() {
        let half = *cfg.parts_per.get(kind) / 2;
        let parts = world.stock(seller, Good::Parts).min(half);
        let import = ((cfg.import_frac * list as f32).round() as i64 - cfg.part_credit * i64::from(parts)).max(0);
        if !can_pay(world, seller_owner, import) {
            return Err(format!("{} cannot pay the import", world.owner_label(seller_owner)));
        }
        // The jobs round (`[economy2] asset_import_per_day`, wages on): a
        // corp fronts at most this much of asset imports a day, so the
        // opening wallets' burst does not ship the city's coins abroad in a
        // week (a refused sale keeps the buyer's coins at home). A corp
        // buyer's purchase (its business fleet: vehicles, robots and decks
        // for its own buildings) is neither capped nor counted; it is paid
        // only from coins above the fleet floor (`wages::fleet_floor`).
        let business = matches!(ownership::owner_kind(world, Some(buyer)), OwnerKind::Corp(_));
        if business && crate::systems::wages::on(world) {
            let floor = crate::systems::wages::fleet_floor(world, buyer);
            if floor > 0 && coins - down < floor {
                return Err(format!(
                    "{} keeps its working capital ({} after the purchase, fleet floor {floor})",
                    world.owner_label(Some(buyer)),
                    coins - down
                ));
            }
        }
        if import > 0 && !business && crate::systems::wages::on(world) {
            let cap = world.config.economy2.asset_import_per_day;
            if let Some(c) = seller_owner.and_then(|o| world.comp::<crate::components::Corp>(o)) {
                // Flip readiness: the day's first import goes through
                // whatever its size (a T1 car's 480 against a 300 cap never
                // sold); the cap refuses the ones after it.
                if cap > 0 && c.import_today > 0 && c.import_today + import > cap {
                    return Err(format!("{} has spent today's import budget", c.name));
                }
                q.capped = true;
            }
        }
        q.parts = parts;
        q.import = import;
    }
    Ok(q)
}

/// OPEN-142: would `buyer`'s sale of `pick` at `seller` go through now?
/// Read before an implant's install wait (and at plan time), so a refused
/// sale (the day's import budget spent, the buyer short) fails at once
/// instead of after 140 minutes on the table.
pub fn sale_ok(world: &World, buyer: EntityId, seller: EntityId, pick: &ShopPick) -> bool {
    quote(world, buyer, seller, pick).is_ok()
}

/// [`buy`] with a note appended to the `AssetBought` text ("for Vat Farm#12").
pub fn buy_noted(
    world: &mut World,
    buyer: EntityId,
    seller: EntityId,
    pick: &ShopPick,
    note: Option<&str>,
) -> Result<EntityId, String> {
    let cfg = world.config.assets.clone();
    let Quote { used, kind, tier, list, price, financed, down, parts, import, capped } =
        quote(world, buyer, seller, pick)?;
    let seller_owner = world.owner_of(seller);
    if used.is_none() {
        if capped {
            if let Some(c) = seller_owner.and_then(|o| world.comp_mut::<crate::components::Corp>(o)) {
                c.import_today += import;
            }
        }
        world.take_stock(seller, Good::Parts, parts);
        ownership::import(world, seller_owner, import, ownership::ImportWhy::Asset);
        // L2 L9: the Fab trigger's tally.
        crate::systems::jobs::note_import(world, seller_owner, import);
    }
    let paid = ownership::charge(world, Some(buyer), seller_owner, down, Flow::Asset);
    ownership::credit(world, seller, paid);
    let loc = new_loc(world, kind, buyer, seller);
    let a = match used {
        Some((u, ..)) => {
            reset_title(world, u);
            set_owner(world, u, Some(buyer));
            set_loc(world, u, loc);
            u
        }
        None => {
            let maker = maker_for(world, seller, kind);
            let a = spawn_asset(world, kind, tier, Some(buyer), loc, list);
            if maker.is_some() {
                if let Some(m) = world.comp_mut::<Asset>(a) {
                    m.maker = maker;
                }
                rekit_touched(world, &touched(world, a));
            }
            a
        }
    };
    if financed {
        let remaining = price - down;
        // Interest in whole coins first, so float noise never adds a coin a day.
        let total = remaining + (remaining as f64 * f64::from(cfg.interest)).round() as i64;
        let term = i64::from(cfg.term_days.max(1));
        let per_day = (total + term - 1) / term;
        let now = world.tick;
        if let Some(m) = world.comp_mut::<Asset>(a) {
            m.finance = Some(Finance { lender: seller_owner, remaining, per_day, arrears: 0 });
            m.bought = now;
        }
    }
    let now = world.tick;
    if let Some(b) = world.comp_mut::<Body>(buyer) {
        b.last_shop = Some(now);
    }
    if let Some(bd) = world.comp_mut::<Building>(seller) {
        bd.asset_sales_today = bd.asset_sales_today.saturating_add(1);
    }
    let (who, what, at) = (world.owner_label(Some(buyer)), world.name_of(a), world.name_of(seller));
    let mut text = format!("{who} bought a {what} at {at} for {price}{}", if financed { " on finance" } else { "" });
    if let Some(n) = note {
        text.push(' ');
        text.push_str(n);
    }
    world.push_event(EventKind::AssetBought, &[buyer, seller, a], text);
    // Lives at work phase 1 (§ 2.2): a Clinic's over-the-counter sale is
    // the doctor's encounter (an implant's install queues in
    // `chrome::installed_now`, once).
    if !matches!(kind, AssetKind::Implant(_))
        && world.comp::<Building>(seller).is_some_and(|bd| bd.kind == BuildingKind::Clinic)
    {
        crate::systems::service::arrive(world, buyer, seller, crate::components::ServeKind::Patient, price);
    }
    Ok(a)
}

/// God (plan D48, phase 1): give `agent` a new asset, free, no import. A
/// vehicle parks at its Home (else the building it is in, else the nearest
/// door), an implant is installed (its slot must be free), a pack or bridge
/// is carried, a robot is posted at its Home.
pub fn grant(world: &mut World, agent: EntityId, kind: AssetKind, tier: u8) -> Result<EntityId, String> {
    if !world.has::<Identity>(agent) || world.has::<Corpse>(agent) {
        return Err("no such living agent".into());
    }
    let list = list_price(world, kind, tier).ok_or_else(|| format!("no {} at tier {tier}", kind.label()))?;
    let home = world.comp::<crate::components::Household>(agent).and_then(|h| h.home);
    let here = world.comp::<Position>(agent).and_then(|p| p.building);
    let near = world.comp::<Position>(agent).and_then(|p| nearest_any_building(world, p.tile));
    let base = home.or(here).or(near);
    let loc = match kind {
        k if k.is_vehicle() => AssetLoc::Parked(base.ok_or("nowhere to park")?),
        AssetKind::Implant(slot) => {
            let taken = assets_at(world, agent).iter().any(|&a| {
                world
                    .comp::<Asset>(a)
                    .is_some_and(|x| x.kind == AssetKind::Implant(slot) && x.loc == AssetLoc::Installed(agent))
            });
            if taken {
                return Err(format!("{} already has {} chrome", world.name_of(agent), slot.label()));
            }
            AssetLoc::Installed(agent)
        }
        AssetKind::Robot => AssetLoc::Posted(base.ok_or("nowhere to post it")?),
        _ => AssetLoc::Carried(agent),
    };
    Ok(spawn_asset(world, kind, tier, Some(agent), loc, list))
}

/// Living agents with a chromed Kit, and the mean sanity of living adults
/// with a Body (the D49 snapshots).
pub fn snapshot(world: &World) -> (u32, f32, u32, [u32; 4]) {
    let mut chrome = 0;
    let (mut sanity, mut n) = (0.0f32, 0u32);
    for &id in world
        .tier(crate::components::Lod::Full)
        .iter()
        .chain(world.tier(crate::components::Lod::Coarse))
        .chain(world.tier(crate::components::Lod::Statistical))
    {
        if world.comp::<Kit>(id).is_some_and(|k| k.chrome) {
            chrome += 1;
        }
        if let Some(b) = world.comp::<Body>(id) {
            if crate::systems::demography::is_adult(world, id) {
                sanity += b.sanity;
                n += 1;
            }
        }
    }
    let mut vehicles = [0u32; 4];
    for &v in &world.vehicles {
        if let Some(x) = world.comp::<Asset>(v) {
            let i = match x.kind {
                AssetKind::Motorcycle => 0,
                AssetKind::Car => 1,
                AssetKind::Truck => 2,
                _ => 3,
            };
            vehicles[i] += 1;
        }
    }
    let robots = all_assets(world)
        .into_iter()
        .filter(|&a| {
            world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Robot && matches!(x.loc, AssetLoc::Posted(_)))
        })
        .count() as u32;
    (chrome, if n > 0 { sanity / n as f32 } else { 0.0 }, robots, vehicles)
}

// ---------------------------------------------------------------------------
// Sellers and the Shop (plan D16, D29, D43)
// ---------------------------------------------------------------------------

/// D16: a seller is open while any of its staff (any tier) is on shift now.
pub fn seller_open(world: &World, b: EntityId) -> bool {
    let Some(bd) = world.comp::<Building>(b) else { return false };
    if bd.demolished || bd.derelict || world.is_closed(b) {
        return false;
    }
    let Some(role) = ownership::role_for(bd.kind) else { return false };
    // L2 shadow fixes item 12: a Clinic sells only with its doctor at the
    // counter on a working shift (Ripperdoc#443 sold three Therapy sessions
    // while its doctor wandered Civic on her rest day).
    if bd.kind == BuildingKind::Clinic {
        return crate::systems::fixes::staff_present(world, b, role);
    }
    let tod = world.tick_of_day();
    // The perf pass phase 4: the building's staff (the employer index), not
    // every holder of the role: the same set, read in fewer steps.
    world
        .staff_of(b)
        .iter()
        .any(|&w| world.comp::<Job>(w).is_some_and(|j| j.role == role && j.employer == Some(b) && j.on_shift(tod)))
}

/// What a Shop offer is for (D43).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ShopCategory {
    Vehicle,
    Pack,
    /// Phase 3: an implant bought and installed at a Clinic.
    Chrome,
    /// Phase 3 (D44): the gang's implant reserved for this member, installed
    /// at a Clinic for `install_fee`.
    Install,
    /// M14 V36: a deck, or the carried deck one tier up (`ShopPick.upgrade`).
    Deck,
    /// Jobs & lives phase 3 (section 4): a weapon at a Gunsmith (licensed)
    /// or a dealer's Bar.
    Weapon,
}

/// The Shop goal's best offer for one agent (D43).
#[derive(Clone, Debug)]
pub struct ShopOffer {
    pub pick: ShopPick,
    pub seller: EntityId,
    pub price: i64,
    pub financed: bool,
    pub category: ShopCategory,
    /// Compensated product of the considerations plus `shop_flat`.
    pub score: f32,
    pub considerations: Vec<crate::utility::Consideration>,
}

/// D10's daily payment for `price` bought with `coins` (0 when paid in full).
fn finance_per_day(world: &World, kind: AssetKind, price: i64, coins: i64) -> i64 {
    if coins >= price {
        return 0;
    }
    let cfg = &world.config.assets;
    let down = (cfg.down_frac_of(kind) * price as f32).round() as i64;
    let remaining = price - down;
    let total = remaining + (remaining as f64 * f64::from(cfg.interest)).round() as i64;
    let term = i64::from(cfg.term_days.max(1));
    (total + term - 1) / term
}

/// The nearest seller of `kind` to `from` (ties lower id), open now when
/// `require_open` (a body walks there; the Statistical pass buys remotely).
/// The perf pass phase 4: off the kind's table (`World::near_table`),
/// tested nearest first in the scan's `(door distance, id)` order up to the
/// first that passes, so `seller_open` (a scan of the role's workers) runs
/// only on the sellers nearer than the answer.
pub(crate) fn nearest_seller(world: &World, kind: BuildingKind, from: TilePos, require_open: bool) -> Option<EntityId> {
    if crate::spatial::scans() {
        return nearest_seller_scan(world, kind, from, require_open);
    }
    let pass = |b: EntityId| {
        world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
            && (!require_open || seller_open(world, b))
    };
    world.near_table(kind).nearest_passing(from, pass)
}

/// [`nearest_seller`] by the scan of every seller (the reference).
fn nearest_seller_scan(world: &World, kind: BuildingKind, from: TilePos, require_open: bool) -> Option<EntityId> {
    world
        .buildings_of_kind(kind)
        .iter()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict))
        .filter(|&b| !require_open || seller_open(world, b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(from), b)))
        .min()
        .map(|(_, b)| b)
}

/// The lowest [`seller_level`] any seller of `kind` asks (`None` with no
/// seller). The perf pass phase 4: read once a `(shop_gen, kinds_gen)`
/// (`World::with_shop_memo`): a level moves only with a write to a Building
/// (its owner), a Corp (its levels, or a corp founded or dissolved), or the
/// kind's list, and each bumps one of the two.
pub(crate) fn lowest_level(world: &World, kind: BuildingKind) -> Option<f32> {
    let scan = || world.buildings_of_kind(kind).iter().map(|&b| seller_level(world, b)).min_by(|a, b| a.total_cmp(b));
    if crate::spatial::scans() {
        return scan();
    }
    if let Some(l) = world.with_shop_memo(|m| m.lowest.get(&kind).copied()) {
        return l;
    }
    let l = scan();
    world.with_shop_memo(|m| m.lowest.insert(kind, l));
    l
}

/// Phase 5 (D43 reading): the coins a day a new asset may still take,
/// `max_burden x income` less the upkeep and finance `per_day` of what the
/// agent already owns. The plan tested each candidate alone, so a wage
/// earner financed implant after implant until it defaulted on all of them.
fn burden_room(world: &World, id: EntityId, income: i64) -> f32 {
    let committed: i64 = assets_of(world, Some(id))
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a).map(|x| (a, x)))
        .map(|(a, x)| upkeep_of(world, a) + x.finance.as_ref().map_or(0, |f| f.per_day))
        .sum();
    world.config.shop.max_burden * income as f32 - committed as f32
}

/// The dearest of `options` the agent can buy (D43): `coins >= down_frac x
/// price` (a flyer only in full; phase 2 deviation: a pack too, or the
/// homeless financed 30-coin packs on the dole and were repossessed within
/// the week), and upkeep plus the plain finance `per_day` (not the catch-up
/// due) at most `room`: `max_burden x income` less what the agent's assets
/// already take a day (phase 5; see [`burden_room`]).
fn dearest_affordable(
    world: &World,
    seller: EntityId,
    options: &[(AssetKind, u8)],
    coins: i64,
    room: f32,
) -> Option<(AssetKind, u8, i64, bool)> {
    // M14 V22: only what the seller's tech lets it sell (all with the plane off).
    let sellable: SmallVec<[(AssetKind, u8); 4]> =
        options.iter().copied().filter(|&(k, t)| can_sell(world, seller, k, t)).collect();
    affordable_at(world, seller_level(world, seller), &sellable, coins, room)
}

/// [`dearest_affordable`] at a given price level.
fn affordable_at(
    world: &World,
    level: f32,
    options: &[(AssetKind, u8)],
    coins: i64,
    cap: f32,
) -> Option<(AssetKind, u8, i64, bool)> {
    options
        .iter()
        .filter_map(|&(kind, tier)| {
            let list = list_price(world, kind, tier)?;
            let price = (list as f32 * level).round() as i64;
            let full = coins >= price;
            let ok = if matches!(kind, AssetKind::Flyer | AssetKind::Pack) {
                full
            } else {
                coins >= (world.config.assets.down_frac_of(kind) * price as f32).round() as i64
            };
            let burden = upkeep_for(world, kind, tier) + finance_per_day(world, kind, price, coins);
            (ok && burden as f32 <= cap).then_some((kind, tier, price, !full))
        })
        .max_by_key(|&(_, _, price, _)| price)
}

/// D43: the agent's best affordable offer, or `None` (cooldown, arrears,
/// nothing affordable, no seller). One candidate per category: a vehicle
/// (Motorcycle T1, Car T1, Car T2, Flyer T1) at the nearest Garage for an
/// agent without one, a pack at the nearest Market for the homeless without
/// one. Considerations per spec section 6: the shared GATE, `U(wealth)`
/// Linear{0.6,0.4}, `pride` Linear{0.5,0.5}; a vehicle's `commute`
/// (Manhattan Home to workplace) Logistic{8, 0.5} on `commute / (2 x
/// commute_ref)` (the spec's mid `commute_ref`, normalised); flat
/// `shop_flat`.
pub fn shop_choice(world: &World, id: EntityId, require_open: bool) -> Option<ShopOffer> {
    use crate::utility::curves::{can, urgency, Curve, GATE};
    use crate::utility::Consideration;
    let cfg = &world.config;
    if !cfg.assets.enabled || world.has::<Sentence>(id) {
        return None;
    }
    let body = world.comp::<Body>(id)?;
    // Phase 3 (D44): the gang's implant waiting for this member comes first,
    // off any cooldown.
    if let Some(offer) = install_offer(world, id, require_open) {
        return Some(offer);
    }
    // The pressure pass phase 2 (OPEN-176): a gang's quartermaster buys for
    // its armoury with the gang's purse (his own wallet and cooldown aside).
    if world.jobs2.arms.quarter.contains_key(&id) {
        let from = world.comp::<Position>(id)?.tile;
        let (seller, tier, price, _) = crate::systems::weapons::offer(world, id, from, require_open)?;
        let cs = vec![
            Consideration::new("the gang's errand", can(true), GATE),
            Consideration::new(
                "loyalty",
                world.comp::<crate::components::Personality>(id).map_or(0.5, |p| p.loyalty),
                Curve::Linear { m: 0.4, b: 0.6 },
            ),
        ];
        let pick = ShopPick { kind: AssetKind::Weapon, tier, used: None, upgrade: false };
        return Some(ShopOffer {
            pick,
            seller,
            price,
            financed: false,
            category: ShopCategory::Weapon,
            score: 0.9,
            considerations: cs,
        });
    }
    let cooldown = u64::from(cfg.shop.shop_cooldown_days) * crate::time::TICKS_PER_DAY;
    if body.last_shop.is_some_and(|t| world.tick.saturating_sub(t) < cooldown) {
        return None;
    }
    let coins = world.comp::<Wallet>(id)?.coins;
    if coins <= 0 || !crate::systems::demography::is_adult(world, id) {
        return None;
    }
    let in_arrears = assets_of(world, Some(id))
        .iter()
        .any(|&a| world.comp::<Asset>(a).is_some_and(|x| x.finance.as_ref().is_some_and(|f| f.arrears > 0)));
    if in_arrears {
        return None;
    }
    let home = world.comp::<crate::components::Household>(id).and_then(|h| h.home);
    let job = world.comp::<Job>(id);
    let mut income = job.map_or(i64::from(world.levers.dole_per_day), |j| j.wage_per_day).max(0);
    // Phase 3 (D43 reading): a member's daily income includes the gang's stipend.
    if world.has::<crate::components::GangMember>(id) {
        income += world.config.social.gang_stipend.max(0);
    }
    // Phase 5 (the chrome finance spiral): net of the day's food, so a wage
    // that only feeds its earner finances nothing.
    let meals = cfg.shop.burden_meals;
    if meals > 0.0 {
        income = (income - (meals * world.mean_price() as f32).round() as i64).max(0);
    }
    let room = burden_room(world, id, income);
    let from = home
        .and_then(|h| world.comp::<Building>(h))
        .map(|b| b.door)
        .or_else(|| world.comp::<Position>(id).map(|p| p.tile))?;
    let needs = world.comp::<crate::components::Needs>(id);
    let wealth = urgency(needs.map_or(1.0, |n| n.wealth));
    let pride = world.comp::<crate::components::Personality>(id).map_or(0.5, |p| p.pride);
    let kit = world.comp::<Kit>(id);
    // Cheap gates before any seller's staff is scanned (this runs twice a
    // think): a category is open only when its options are within reach at
    // the lowest price level any seller of the kind asks.
    const VEHICLES: [(AssetKind, u8); 4] =
        [(AssetKind::Motorcycle, 1), (AssetKind::Car, 1), (AssetKind::Car, 2), (AssetKind::Flyer, 1)];
    const PACKS: [(AssetKind, u8); 2] = [(AssetKind::Pack, 1), (AssetKind::Pack, 2)];
    let lowest = |kind: BuildingKind| lowest_level(world, kind);
    let has_pack = assets_at(world, id)
        .iter()
        .any(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Pack && x.loc == AssetLoc::Carried(id)));
    let want_vehicle = kit.is_none_or(|k| k.vehicle.is_none())
        && lowest(BuildingKind::Garage).is_some_and(|l| affordable_at(world, l, &VEHICLES, coins, room).is_some());
    let want_pack = home.is_none()
        && !has_pack
        && lowest(BuildingKind::Market).is_some_and(|l| affordable_at(world, l, &PACKS, coins, room).is_some());
    // Phase 3 (D43): the first empty slot of the agent's list, while a T1
    // is within reach at the cheapest Clinic (a used one may be cheaper).
    let chrome_slot = chrome_slot(world, id);
    // Violence fix 3: the dearest tier the agent's humanity still takes;
    // Shadow V4 bug 13: a creed gang's member no tier past what it allows
    // on a slot that shows.
    let shows = chrome_slot.is_some_and(|s| matches!(s, Slot::Arms | Slot::Eyes | Slot::Skin));
    let max_tier = match crate::systems::creeds::visible_room(world, id) {
        Some(room) if shows => chrome_max_tier(world, id).min(room),
        _ => chrome_max_tier(world, id),
    };
    let want_chrome = max_tier >= 1
        && chrome_slot.is_some_and(|s| {
            lowest(BuildingKind::Clinic)
                .is_some_and(|l| affordable_at(world, l, &[(AssetKind::Implant(s), 1)], coins, room).is_some())
        });
    // M14 V36: a deck for a hacker without one (GATE `hacking >= deck_shop_min`),
    // or the carried deck one tier up; at Clinics and Security Offices (V24).
    let hacking = world.comp::<crate::components::Skills>(id).map_or(0.0, |s| s.hacking);
    let deck_on = hacking >= cfg.decks.deck_shop_min;
    const DECKS: [(AssetKind, u8); 3] = [(AssetKind::Deck, 1), (AssetKind::Deck, 2), (AssetKind::Deck, 3)];
    let deck_level = || {
        [BuildingKind::Clinic, BuildingKind::SecurityOffice]
            .into_iter()
            .filter_map(lowest)
            .min_by(|a, b| a.total_cmp(b))
    };
    let own_deck = kit
        .and_then(|k| k.deck)
        .and_then(|d| world.comp::<Asset>(d).filter(|x| x.owner == Some(id)).map(|x| (d, x.tier)));
    let want_deck = deck_on
        && kit.is_none_or(|k| k.deck.is_none())
        && deck_level().is_some_and(|l| affordable_at(world, l, &DECKS, coins, room).is_some());
    let want_upgrade = deck_on
        && own_deck.is_some_and(|(_, t)| {
            t < 3 && deck_level().is_some_and(|l| upgrade_price(world, t, l).is_some_and(|p| coins >= p))
        });
    // Jobs & lives phase 3 (section 4): a weapon, for an adult who wants one
    // (`None` with `[weapons]` off).
    let weapon = crate::systems::weapons::offer(world, id, from, require_open);
    if !want_vehicle && !want_pack && !want_chrome && !want_deck && !want_upgrade && weapon.is_none() {
        return None;
    }
    let mut best: Option<ShopOffer> = None;
    let mut offer = |category: ShopCategory,
                     seller: EntityId,
                     (kind, tier, price, financed): (AssetKind, u8, i64, bool),
                     used: Option<EntityId>,
                     upgrade: bool,
                     mut cs: Vec<Consideration>| {
        cs.insert(0, Consideration::new("can buy", can(true), GATE));
        cs.insert(1, Consideration::new("U(wealth)", wealth, Curve::Linear { m: 0.6, b: 0.4 }));
        cs.insert(2, Consideration::new("pride", pride, Curve::Linear { m: 0.5, b: 0.5 }));
        if cs.iter().any(|c| c.output <= 0.0) {
            return;
        }
        let raw: f32 = cs.iter().map(|c| c.output).product();
        let score = crate::utility::compensate(raw, cs.len()) + cfg.shop.shop_flat;
        if best.as_ref().is_none_or(|b| score > b.score) {
            let pick = ShopPick { kind, tier, used, upgrade };
            best = Some(ShopOffer { pick, seller, price, financed, category, score, considerations: cs });
        }
    };
    // A vehicle, for an agent without one.
    if want_vehicle {
        if let Some(g) = nearest_seller(world, BuildingKind::Garage, from, require_open) {
            if let Some(o) = dearest_affordable(world, g, &VEHICLES, coins, room) {
                let work = job.and_then(|j| j.employer).and_then(|e| world.comp::<Building>(e)).map(|b| b.door);
                let commute = match (home, work) {
                    (Some(_), Some(w)) => from.manhattan(w),
                    _ => 0,
                };
                let x = commute as f32 / (2.0 * cfg.shop.commute_ref.max(1) as f32);
                let cs = vec![Consideration::new("commute", x, Curve::Logistic { k: 8.0, mid: 0.5 })];
                offer(ShopCategory::Vehicle, g, o, None, false, cs);
            }
        }
    }
    // Chrome: the dearest affordable tier of the slot, new or used.
    if let (true, Some(slot)) = (want_chrome, chrome_slot) {
        if let Some(c) = nearest_seller(world, BuildingKind::Clinic, from, require_open) {
            if let Some((pick, price, financed)) = chrome_pick(world, c, slot, coins, room, max_tier) {
                let load = kit.map_or(0.0, |k| k.load);
                let courage = world.comp::<crate::components::Personality>(id).map_or(0.5, |p| p.courage);
                let cs = vec![
                    Consideration::new("courage", courage, Curve::Linear { m: 0.5, b: 0.5 }),
                    Consideration::new("1-load", (1.0 - load).clamp(0.0, 1.0), Curve::Linear { m: 0.6, b: 0.4 }),
                ];
                offer(ShopCategory::Chrome, c, (pick.kind, pick.tier, price, financed), pick.used, false, cs);
            }
        }
    }
    // Jobs & lives phase 3: the weapon (the unlicensed buyer's lawfulness weighs it).
    if let Some((seller, tier, price, legal)) = weapon {
        let p = world.comp::<crate::components::Personality>(id);
        let courage = p.map_or(0.5, |p| p.courage);
        let mut cs = vec![Consideration::new("courage", courage, Curve::Linear { m: 0.5, b: 0.5 })];
        if !legal {
            let law = p.map_or(0.5, |p| p.lawfulness);
            cs.push(Consideration::new("1-lawfulness", 1.0 - law, Curve::Linear { m: 0.5, b: 0.5 }));
        }
        offer(ShopCategory::Weapon, seller, (AssetKind::Weapon, tier, price, false), None, false, cs);
    }
    // A pack, for the homeless without one.
    if want_pack {
        if let Some(m) = nearest_seller(world, BuildingKind::Market, from, require_open) {
            if let Some(o) = dearest_affordable(world, m, &PACKS, coins, room) {
                offer(ShopCategory::Pack, m, o, None, false, Vec::new());
            }
        }
    }
    // M14 V36: the dearest affordable deck at the nearest Clinic or Office
    // (ties the Clinic), or the carried deck's upgrade where it can be sold.
    if want_deck || want_upgrade {
        let law = world.comp::<crate::components::Personality>(id).map_or(0.5, |p| p.lawfulness);
        let cs = || {
            vec![
                Consideration::new("hacking", hacking, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1-lawfulness", 1.0 - law, Curve::Linear { m: 0.5, b: 0.5 }),
            ]
        };
        let sellers: SmallVec<[EntityId; 2]> = [BuildingKind::Clinic, BuildingKind::SecurityOffice]
            .into_iter()
            .filter_map(|k| nearest_seller(world, k, from, require_open))
            .collect();
        if want_deck {
            let mut best_deck: Option<(EntityId, (AssetKind, u8, i64, bool))> = None;
            for &s in &sellers {
                if let Some(o) = dearest_affordable(world, s, &DECKS, coins, room) {
                    if best_deck.is_none_or(|b| o.2 > b.1 .2) {
                        best_deck = Some((s, o));
                    }
                }
            }
            if let Some((s, o)) = best_deck {
                offer(ShopCategory::Deck, s, o, None, false, cs());
            }
        }
        if let (true, Some((_, tier))) = (want_upgrade, own_deck) {
            let up = sellers.iter().find_map(|&s| {
                let price = upgrade_price(world, tier, seller_level(world, s))?;
                (coins >= price && can_sell(world, s, AssetKind::Deck, tier + 1)).then_some((s, price))
            });
            if let Some((s, price)) = up {
                offer(ShopCategory::Deck, s, (AssetKind::Deck, tier + 1, price, false), None, true, cs());
            }
        }
    }
    best
}

/// M14 V36: what raising a deck from `tier` to `tier + 1` costs at a price
/// level: `(price[t + 1] - price[t]) x upgrade_frac x level`.
pub fn upgrade_price(world: &World, tier: u8, level: f32) -> Option<i64> {
    let (now, next) = (list_price(world, AssetKind::Deck, tier)?, list_price(world, AssetKind::Deck, tier + 1)?);
    Some(((next - now) as f32 * world.config.decks.upgrade_frac * level).round() as i64)
}

/// M14 V24: what a seller building sells new (Clinic: implants and decks;
/// Office: robots, decks, cameras; Garage: vehicles; Market: packs).
pub fn sells(kind: BuildingKind) -> &'static [AssetKind] {
    match kind {
        BuildingKind::Clinic => &[
            AssetKind::Implant(Slot::Arms),
            AssetKind::Implant(Slot::Legs),
            AssetKind::Implant(Slot::Nerves),
            AssetKind::Implant(Slot::Eyes),
            AssetKind::Implant(Slot::Skin),
            AssetKind::Deck,
        ],
        BuildingKind::SecurityOffice => &[AssetKind::Robot, AssetKind::Deck, AssetKind::Camera],
        BuildingKind::Garage => &[AssetKind::Motorcycle, AssetKind::Car, AssetKind::Truck, AssetKind::Flyer],
        BuildingKind::Market => &[AssetKind::Pack],
        _ => &[],
    }
}

/// M14 V36 `UpgradeDeck`: the agent's own carried deck one tier up at
/// `seller` (which must sell the new tier): `upgrade_price` in full to the
/// seller's owner (`Flow::Asset`, taxed); the deck's tier, list, value and
/// upkeep move up, its maker becomes the seller's. `AssetBought`.
pub fn upgrade_deck(world: &mut World, agent: EntityId, seller: EntityId, pick: &ShopPick) -> Result<(), String> {
    let deck = world.comp::<Kit>(agent).and_then(|k| k.deck).ok_or("no deck")?;
    let tier = world
        .comp::<Asset>(deck)
        .filter(|x| x.owner == Some(agent) && x.kind == AssetKind::Deck)
        .map(|x| x.tier)
        .ok_or("not the agent's deck")?;
    let sold_here = world.comp::<Building>(seller).is_some_and(|b| sells(b.kind).contains(&AssetKind::Deck));
    if !sold_here || pick.tier != tier + 1 || !can_sell(world, seller, AssetKind::Deck, pick.tier) {
        return Err(format!("{} cannot raise a deck to T{}", world.name_of(seller), pick.tier));
    }
    let price = upgrade_price(world, tier, seller_level(world, seller)).ok_or("no such tier")?;
    if world.purse(Some(agent)) < price {
        return Err("cannot afford the upgrade".into());
    }
    let owner = world.owner_of(seller);
    let paid = ownership::charge(world, Some(agent), owner, price, Flow::Asset);
    ownership::credit(world, seller, paid);
    let list = list_price(world, AssetKind::Deck, pick.tier).unwrap_or(0);
    let upkeep = upkeep_for(world, AssetKind::Deck, pick.tier);
    let maker = maker_for(world, seller, AssetKind::Deck);
    if let Some(x) = world.comp_mut::<Asset>(deck) {
        x.tier = pick.tier;
        x.list = list;
        x.value = list;
        x.upkeep_per_day = upkeep;
        x.maker = maker;
    }
    rekit(world, agent);
    let now = world.tick;
    if let Some(b) = world.comp_mut::<Body>(agent) {
        b.last_shop = Some(now);
    }
    if let Some(bd) = world.comp_mut::<Building>(seller) {
        bd.asset_sales_today = bd.asset_sales_today.saturating_add(1);
    }
    let text =
        format!("{} upgraded a deck to T{} at {} for {price}", world.name_of(agent), pick.tier, world.name_of(seller));
    world.push_event(EventKind::AssetBought, &[agent, seller, deck], text);
    Ok(())
}

/// Violence fix 3: a gang member, a guard or a pit Fighter: the roles that
/// fight for a living (the five-slot list, and no humanity cap).
fn fights_for_a_living(world: &World, id: EntityId) -> bool {
    world.has::<crate::components::GangMember>(id)
        || crate::systems::law::is_guard(world, id)
        || world.comp::<Job>(id).is_some_and(|j| j.role == crate::components::Role::Fighter)
}

/// Violence fix 3: the dearest implant tier `id` may buy: 3 (no cap) with
/// the fix off or for a gang member, a guard or a Fighter; else the
/// dearest tier whose `[chrome] sanity_cost` added to `Kit.load` keeps the
/// target sanity (`1 - load`) at or above `psycho`, 0 when none does. Two
/// T1s (load 0.16) stay over 0.8; a third (0.24) would cross it.
pub fn chrome_max_tier(world: &World, id: EntityId) -> u8 {
    if !crate::systems::fixes::chrome_cap(world) || fights_for_a_living(world, id) {
        return 3;
    }
    let cfg = &world.config.chrome;
    let load = world.comp::<Kit>(id).map_or(0.0, |k| k.load);
    // A float margin: 1 - (0.08 + 0.08 + 0.04) is not exactly 0.8.
    let room = 1.0 - cfg.psycho - load + 1e-4;
    (1..=3u8).rev().find(|&t| cfg.sanity_cost.get(usize::from(t - 1)).copied().unwrap_or(0.0) <= room).unwrap_or(0)
}

/// Phase 3 (D43): the slot an agent would chrome next: the first empty one
/// of `Arms, Nerves, Skin, Eyes, Legs` for a gang member, a guard or
/// `courage >= 0.6` (violence fix 3: a Fighter instead of the courage),
/// else of `Legs, Eyes`.
pub fn chrome_slot(world: &World, id: EntityId) -> Option<Slot> {
    const FIGHTER: [Slot; 5] = [Slot::Arms, Slot::Nerves, Slot::Skin, Slot::Eyes, Slot::Legs];
    const CIVILIAN: [Slot; 2] = [Slot::Legs, Slot::Eyes];
    let fighter = if crate::systems::fixes::chrome_cap(world) {
        fights_for_a_living(world, id)
    } else {
        world.has::<crate::components::GangMember>(id)
            || crate::systems::law::is_guard(world, id)
            || world.comp::<crate::components::Personality>(id).is_some_and(|p| p.courage >= 0.6)
    };
    let list: &[Slot] = if fighter { &FIGHTER } else { &CIVILIAN };
    let taken: SmallVec<[Slot; 5]> = assets_at(world, id)
        .iter()
        .filter_map(|&a| world.comp::<Asset>(a))
        .filter(|x| x.loc == AssetLoc::Installed(id))
        .filter_map(|x| match x.kind {
            AssetKind::Implant(s) => Some(s),
            _ => None,
        })
        .collect();
    list.iter().copied().find(|s| !taken.contains(s))
}

/// Phase 3 (D43): the dearest implant for `slot` at Clinic `c` the agent
/// can buy: a new tier (`dearest_affordable`'s rules), or a used one in the
/// Clinic's stock at `used_frac × list × condition / 100`; no tier above
/// `max_tier` (violence fix 3, `chrome_max_tier`).
fn chrome_pick(
    world: &World,
    c: EntityId,
    slot: Slot,
    coins: i64,
    cap: f32,
    max_tier: u8,
) -> Option<(ShopPick, i64, bool)> {
    let kind = AssetKind::Implant(slot);
    let tiers = [(kind, 1), (kind, 2), (kind, 3)];
    let new = dearest_affordable(world, c, &tiers[..usize::from(max_tier.min(3))], coins, cap)
        .map(|(k, t, price, fin)| (ShopPick { kind: k, tier: t, used: None, upgrade: false }, price, fin));
    let down_frac = world.config.assets.down_frac_of(kind);
    let used = assets_at(world, c)
        .iter()
        .copied()
        .filter_map(|a| world.comp::<Asset>(a).map(|x| (a, x)))
        .filter(|(_, x)| {
            x.kind == kind
                && x.tier <= max_tier
                && x.loc == AssetLoc::Stock(c)
                && x.condition > 0
                && x.owner == world.owner_of(c)
        })
        .filter_map(|(a, x)| {
            let price = (world.config.chrome.used_frac * x.list as f32 * f32::from(x.condition) / 100.0).round() as i64;
            let ok = coins >= (down_frac * price as f32).round() as i64;
            let burden = upkeep_for(world, kind, x.tier) + finance_per_day(world, kind, price, coins);
            (ok && burden as f32 <= cap).then_some((
                ShopPick { kind, tier: x.tier, used: Some(a), upgrade: false },
                price,
                coins < price,
            ))
        })
        .max_by_key(|(p, price, _)| (*price, std::cmp::Reverse(p.used)));
    match (new, used) {
        (Some(n), Some(u)) => Some(if u.1 > n.1 { u } else { n }),
        (n, u) => n.or(u),
    }
}

/// Phase 3 (D44): the install offer for a member with a gang implant
/// reserved: at the nearest Clinic (open, for a body), considerations the
/// shared gate and the member's loyalty and courage.
fn install_offer(world: &World, id: EntityId, require_open: bool) -> Option<ShopOffer> {
    use crate::utility::curves::{can, Curve, GATE};
    use crate::utility::Consideration;
    let a = crate::systems::chrome::reserved_implant(world, id)?;
    let x = world.comp::<Asset>(a)?;
    let from = world.comp::<Position>(id)?.tile;
    let seller = nearest_seller(world, BuildingKind::Clinic, from, require_open)?;
    let p = world.comp::<crate::components::Personality>(id);
    let cs = vec![
        Consideration::new("gang chrome", can(true), GATE),
        Consideration::new("loyalty", p.map_or(0.5, |p| p.loyalty), Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new("courage", p.map_or(0.5, |p| p.courage), Curve::Linear { m: 0.5, b: 0.5 }),
    ];
    let raw: f32 = cs.iter().map(|c| c.output).product();
    let score = crate::utility::compensate(raw, cs.len()) + world.config.shop.shop_flat;
    Some(ShopOffer {
        pick: ShopPick { kind: x.kind, tier: x.tier, used: Some(a), upgrade: false },
        seller,
        price: crate::systems::chrome::install_fee(world, x.tier),
        financed: false,
        category: ShopCategory::Install,
        score,
        considerations: cs,
    })
}

/// D43, daily: Statistical adults on their day in seven (`(index + day) %
/// 7 == 0`) score the same offer at the nearest seller, open or not (the
/// pass runs at midnight; deviation), and buy remotely at `stat_shop_min`
/// or better; a vehicle is parked at the buyer's Home.
pub(super) fn stat_shop(world: &mut World) {
    let day = world.day();
    let min = world.config.shop.stat_shop_min;
    let due: Vec<EntityId> = world
        .tier(Lod::Statistical)
        .iter()
        .copied()
        .filter(|id| (u64::from(id.index) + day).is_multiple_of(7))
        // L2 (L21): a held prisoner does not shop from the cells.
        .filter(|&id| !world.has::<crate::components::Sentence>(id))
        .collect();
    for id in due {
        let Some(o) = shop_choice(world, id, false) else { continue };
        if o.score < min {
            continue;
        }
        // Phase 3 (D43): Statistical chrome is installed in place.
        // Round 5 U6 (OPEN-217): booked for an hour of the clinic's day
        // (`care::book`; installed in place without `[care] book_hours`).
        if o.category == ShopCategory::Chrome {
            if !crate::systems::care::book(world, id, o.seller, &o.pick) {
                crate::systems::chrome::stat_install(world, id, o.seller, &o.pick);
            }
            continue;
        }
        if o.category == ShopCategory::Install {
            continue;
        }
        // Jobs & lives phase 3: a weapon off screen (a Gunsmith's stock, a
        // dealer's surplus).
        if o.category == ShopCategory::Weapon {
            // The pre-view pass phase 4 (OPEN-119): a dealer who is a body
            // sells at the meeting, not at midnight.
            if crate::systems::weapons::book_meeting(world, id, o.seller, &o.pick) {
                continue;
            }
            let _ = crate::systems::weapons::buy(world, id, o.seller, &o.pick, false);
            continue;
        }
        let Ok(a) = buy(world, id, o.seller, &o.pick) else { continue };
        if o.category == ShopCategory::Vehicle {
            if let Some(h) = world.comp::<crate::components::Household>(id).and_then(|h| h.home) {
                set_loc(world, a, AssetLoc::Parked(h));
            }
        }
    }
}
