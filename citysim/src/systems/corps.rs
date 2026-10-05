//! Corp mechanics the brain and the levers call (docs/M11_OWNERSHIP.md § 5,
//! plan D19, D28-D31): security contracts, acquisition, bankruptcy and
//! dissolution, monopoly and `BreakUp`, and the daily pass that bills
//! contracts, renews them weekly, rolls the share history, sends Undercut
//! shocks, bankrupts corps that stayed in the red and counts monopolies.

use std::collections::BTreeMap;

use crate::components::{Building, BuildingKind, Corp, CorpOrder, CorpShock, Niche, Personality, Role};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::corp_brain;
use crate::systems::ownership::{self, Flow, OwnerKind};
use crate::time::TICKS_PER_DAY;
use crate::world::World;

/// `share ≥ this` is a monopoly (D16).
pub const MONOPOLY_SHARE: f32 = 0.999;
/// `Corp.share_hist` keeps today and the seven days before.
const SHARE_HIST: usize = 8;
/// Contracts renew weekly.
const CONTRACT_DAYS: u64 = 7;

/// Does `corp` hold a monopoly in `niche`?
pub fn is_monopoly(world: &World, corp: EntityId, niche: Niche) -> bool {
    corp_brain::shares(world, niche).get(&corp).is_some_and(|&s| s >= MONOPOLY_SHARE)
}

/// What one contract costs a day from this seller (D19).
pub fn contract_price(world: &World, seller: EntityId) -> i64 {
    let level = world.comp::<Corp>(seller).map_or(1.0, |c| c.level(Niche::Security));
    (world.config.corps.contract_per_guard_day as f32 * level).round() as i64
}

/// How many contracts a Security corp can hold: `security_guards` per owned
/// Security Office (one guard a client; plan D16 reads demand the same way).
pub fn capacity(world: &World, seller: EntityId) -> usize {
    let offices = world.comp::<Corp>(seller).map_or(0, |c| {
        c.buildings
            .iter()
            .filter(|&&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::SecurityOffice))
            .count()
    });
    offices * world.config.corps.security_guards as usize
}

fn has_room(world: &World, seller: EntityId) -> bool {
    world.comp::<Corp>(seller).is_some_and(|c| c.contracts.len() < capacity(world, seller))
}

/// The cheapest Security corp with room, `not` excluded; at the same price
/// the one with fewer contracts (its guards are freer), then the lower id.
pub fn cheapest_seller(world: &World, not: Option<EntityId>) -> Option<EntityId> {
    world
        .corps()
        .into_iter()
        .filter(|&c| Some(c) != not)
        .filter(|&c| world.comp::<Corp>(c).is_some_and(|cc| cc.niches.contains(&Niche::Security)))
        .filter(|&c| has_room(world, c))
        .map(|c| (contract_price(world, c), world.comp::<Corp>(c).map_or(0, |cc| cc.contracts.len()), c))
        .min()
        .map(|(_, _, c)| c)
}

/// A new weekly contract on `client` with `seller` (D19). Refused when the
/// client is already secured or the seller is full.
pub fn buy_contract(world: &mut World, client: EntityId, seller: EntityId) -> bool {
    if world.comp::<Building>(client).is_none_or(|b| b.secured_by.is_some() || b.demolished) {
        return false;
    }
    if !has_room(world, seller) {
        return false;
    }
    let until = world.tick + CONTRACT_DAYS * TICKS_PER_DAY;
    if let Some(c) = world.comp_mut::<Corp>(seller) {
        if let Err(i) = c.contracts.binary_search_by_key(&client, |&(b, _)| b) {
            c.contracts.insert(i, (client, until));
        }
    }
    if let Some(b) = world.comp_mut::<Building>(client) {
        b.secured_by = Some(seller);
    }
    let price = contract_price(world, seller);
    let owner = world.owner_label(world.owner_of(client));
    let text =
        format!("{} now guards {} for {owner} ({price}/day)", world.owner_label(Some(seller)), world.name_of(client));
    world.push_event(EventKind::Contract, &[seller, client], text);
    true
}

/// End the contract on `client`, if any (D19).
pub fn end_contract(world: &mut World, client: EntityId, why: &str) {
    let Some(seller) = world.comp::<Building>(client).and_then(|b| b.secured_by) else { return };
    if let Some(c) = world.comp_mut::<Corp>(seller) {
        c.contracts.retain(|&(b, _)| b != client);
    }
    if let Some(b) = world.comp_mut::<Building>(client) {
        b.secured_by = None;
    }
    let text = format!("{} no longer guards {} ({why})", world.owner_label(Some(seller)), world.name_of(client));
    world.push_event(EventKind::Contract, &[seller, client], text);
}

/// D30 plus the Office rule: a building changes hands; a Security Office
/// takes its seller's contracts along when the seller has no other office.
/// A corp buying outside its niches gains the niche (at level 1.0).
pub fn move_building(world: &mut World, b: EntityId, to: Option<EntityId>) {
    let Some((kind, from)) = world.comp::<Building>(b).map(|bd| (bd.kind, bd.owner)) else { return };
    ownership::transfer_building(world, b, to);
    if let (Some(n), Some(t)) = (corp_brain::niche_of_kind(kind), to) {
        if let Some(c) = world.comp_mut::<Corp>(t) {
            if c.niches.insert(n) {
                c.price_level.entry(n).or_insert(1.0);
            }
        }
    }
    if kind != BuildingKind::SecurityOffice {
        return;
    }
    let Some(seller) = from.filter(|&f| world.has::<Corp>(f)) else { return };
    if capacity(world, seller) > 0 {
        return;
    }
    let contracts = world.comp_mut::<Corp>(seller).map(|c| std::mem::take(&mut c.contracts)).unwrap_or_default();
    match to.filter(|&t| world.has::<Corp>(t)) {
        Some(buyer) => {
            for &(client, _) in &contracts {
                if let Some(cb) = world.comp_mut::<Building>(client) {
                    cb.secured_by = Some(buyer);
                }
            }
            if let Some(c) = world.comp_mut::<Corp>(buyer) {
                c.contracts.extend(contracts);
                c.contracts.sort_by_key(|&(b, _)| b);
                c.contracts.dedup_by_key(|&mut (b, _)| b);
            }
        }
        None => {
            for (client, _) in contracts {
                if let Some(cb) = world.comp_mut::<Building>(client) {
                    cb.secured_by = None;
                }
            }
        }
    }
}

/// `buyer` pays `price` for `building` (`Flow::Sale`) and takes it, with its
/// employees, vacancies and contract (D30). `Acquired` event with `why`.
pub fn acquire(world: &mut World, buyer: EntityId, building: EntityId, price: i64, why: &str) -> bool {
    let Some(seller) = world.comp::<Building>(building).map(|b| b.owner) else { return false };
    if seller == Some(buyer) || world.purse(Some(buyer)) < price {
        return false;
    }
    let seller_name = world.owner_label(seller);
    ownership::pay(world, Some(buyer), seller, price, Flow::Sale);
    move_building(world, building, Some(buyer));
    let mut actors = vec![buyer];
    actors.extend(seller);
    actors.push(building);
    let text = format!(
        "{} bought {} from {seller_name} for {price} ({why})",
        world.owner_label(Some(buyer)),
        world.name_of(building)
    );
    world.push_event(EventKind::Acquired, &actors, text);
    world.stats.current.acquisitions += 1;
    true
}

/// D29: a corp in the red for `bankrupt_days` (or with nothing left) sells
/// every building, ascending, to the highest purse among the other corps
/// and living agents that can pay its value, else to the city at half; the
/// estate settles and the corp is dissolved.
pub fn bankrupt(world: &mut World, corp: EntityId) {
    let Some((name, treasury, buildings, exec)) =
        world.comp::<Corp>(corp).map(|c| (c.name.clone(), c.treasury, c.buildings.clone(), c.exec))
    else {
        return;
    };
    let mut actors = vec![corp];
    actors.extend(exec);
    world.push_event(
        EventKind::Bankrupt,
        &actors,
        format!("{name} went bankrupt ({treasury} in the treasury, {} buildings to sell)", buildings.len()),
    );
    world.stats.current.bankruptcies += 1;
    // Buyers: other corps and living agents, by what they can spend: an
    // agent its wallet, a corp what it holds above its reserve
    // (`treasury_ref`: a corp does not empty its own treasury for a fire
    // sale; the richest corp bought every Sump Block in the city and starved
    // its Farms). Updated as they buy.
    let mut buyers: Vec<(i64, EntityId)> = world
        .corps()
        .into_iter()
        .filter(|&c| c != corp)
        .filter_map(|c| world.comp::<Corp>(c).map(|cc| (cc.treasury - cc.treasury_ref, c)))
        .collect();
    buyers.extend(corp_brain::wallets(world));
    for b in buildings {
        let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { continue };
        let v = ownership::value(world, kind);
        let pick = if v > 0 {
            buyers
                .iter()
                .enumerate()
                .filter(|(_, &(p, _))| p >= v)
                .max_by(|a, b| a.1 .0.cmp(&b.1 .0).then(b.1 .1.cmp(&a.1 .1)))
                .map(|(k, &(_, id))| (k, id))
        } else {
            None
        };
        let what = world.name_of(b);
        match pick {
            Some((k, buyer)) => {
                ownership::pay(world, Some(buyer), Some(corp), v, Flow::Sale);
                buyers[k].0 -= v;
                move_building(world, b, Some(buyer));
                let text = format!("{} bought {what} from bankrupt {name} for {v}", world.owner_label(Some(buyer)));
                world.push_event(EventKind::Acquired, &[buyer, corp, b], text);
            }
            None => {
                let half = v / 2;
                ownership::charge(world, None, Some(corp), half, Flow::Sale);
                move_building(world, b, None);
                // The city does not keep a corp's guard contract.
                end_contract(world, b, "sold to the city");
                let text = format!("the city took {what} from bankrupt {name} for {half}");
                world.push_event(EventKind::Acquired, &[corp, b], text);
            }
        }
        world.stats.current.acquisitions += 1;
    }
    if let Some(p) = exec.and_then(|e| world.comp_mut::<Personality>(e)) {
        p.greed = (p.greed + 0.05).min(1.0);
    }
    for other in world.corps() {
        if other != corp {
            corp_brain::push_shock(world, other, CorpShock::Bankrupt(corp));
        }
    }
    dissolve(world, corp);
}

/// Settle the estate and despawn: a positive balance goes to the exec (else
/// the city), a negative one is absorbed by the Treasury; the corp's sold
/// contracts end, a lobby hold naming it lapses, anything still owned goes
/// to the city.
pub fn dissolve(world: &mut World, corp: EntityId) {
    let Some(c) = world.comp::<Corp>(corp) else { return };
    let (left, clients, exec) = (c.buildings.clone(), c.contracts.clone(), c.exec);
    for b in left {
        move_building(world, b, None);
    }
    for (client, _) in clients {
        end_contract(world, client, "the guards' corp is gone");
    }
    let balance = world.purse(Some(corp));
    if balance > 0 {
        let heir = exec.filter(|&e| matches!(ownership::owner_kind(world, Some(e)), OwnerKind::Agent(_)));
        ownership::pay(world, Some(corp), heir, balance, Flow::Sale);
    } else if balance < 0 {
        ownership::charge(world, None, Some(corp), -balance, Flow::Sale);
    }
    if let Some(l) = world.law_mut() {
        if l.lobby.is_some_and(|h| h.corp == corp) {
            l.lobby = None;
        }
    }
    world.despawn(corp);
}

/// D31: split a monopoly. Every second niche building (ascending id, from
/// the second) moves to a new corp "{name} Spinoff" holding only that niche,
/// run by the second-greediest adult employee, with the treasury share of
/// the buildings it takes.
pub fn break_up(world: &mut World, corp: EntityId) -> Result<EntityId, String> {
    let Some(c) = world.comp::<Corp>(corp) else { return Err("not a corp".into()) };
    let (name, niches, owned, treasury) = (c.name.clone(), c.niches.clone(), c.buildings.len(), c.treasury);
    let Some(niche) = niches.iter().copied().find(|&n| is_monopoly(world, corp, n)) else {
        return Err(format!("{name} is not a monopoly anywhere"));
    };
    let moved: Vec<EntityId> = corp_brain::niche_buildings(world, corp, niche).into_iter().skip(1).step_by(2).collect();
    if moved.is_empty() {
        return Err(format!("{name} has one {niche} building; nothing to split"));
    }
    // The second-greediest adult employee who is nobody's exec (ties lower id).
    let buildings: Vec<EntityId> = world.comp::<Corp>(corp).map(|c| c.buildings.clone()).unwrap_or_default();
    let execs: Vec<EntityId> =
        world.corps().into_iter().filter_map(|c| world.comp::<Corp>(c).and_then(|c| c.exec)).collect();
    let mut staff: Vec<(f32, EntityId)> = Vec::new();
    for role in Role::ALL {
        for &a in world.workers(role) {
            let here = world
                .comp::<crate::components::Job>(a)
                .and_then(|j| j.employer)
                .is_some_and(|e| buildings.binary_search(&e).is_ok());
            if here && !execs.contains(&a) && crate::systems::demography::is_adult(world, a) {
                staff.push((world.comp::<Personality>(a).map_or(0.0, |p| p.greed), a));
            }
        }
    }
    staff.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let exec = staff.get(1).or(staff.first()).map(|&(_, a)| a);
    let spin_name = format!("{name} Spinoff");
    let spinoff = ownership::spawn_corp(world, spin_name.clone(), [niche].into_iter().collect(), 0, exec);
    let amount = (treasury.max(0) as f64 * moved.len() as f64 / owned.max(1) as f64).round() as i64;
    if amount > 0 {
        ownership::charge(world, Some(corp), Some(spinoff), amount, Flow::Sale);
    }
    if let Some(s) = world.comp_mut::<Corp>(spinoff) {
        s.treasury_ref = amount.max(1000);
    }
    for &b in &moved {
        move_building(world, b, Some(spinoff));
    }
    world.push_event(
        EventKind::BrokenUp,
        &[corp, spinoff],
        format!("{name} broken up in {niche}: {} buildings and {amount} coins to {spin_name}", moved.len()),
    );
    Ok(spinoff)
}

/// Daily at midnight, after the brain acted (D19, D16, D29).
pub fn daily(world: &mut World) {
    bill_contracts(world);
    renew_contracts(world);
    roll_shares(world);
    bankruptcies(world);
    count_monopolies(world);
}

/// Each contract bills its client's owner; a short payment ends it. A
/// Security corp guards its own buildings free; the city never pays.
fn bill_contracts(world: &mut World) {
    for seller in world.corps() {
        let Some(contracts) = world.comp::<Corp>(seller).map(|c| c.contracts.clone()) else { continue };
        let price = contract_price(world, seller);
        for (client, _) in contracts {
            let owner = world.comp::<Building>(client).filter(|b| !b.demolished).map(|b| b.owner);
            match owner {
                None => end_contract(world, client, "the building is gone"),
                Some(None) => end_contract(world, client, "the city does not pay"),
                Some(Some(o)) if o == seller => {}
                Some(Some(o)) => {
                    let paid = ownership::pay(world, Some(o), Some(seller), price, Flow::Contract);
                    if paid < price {
                        end_contract(world, client, "unpaid");
                    }
                }
            }
        }
    }
}

/// Weekly renewal: a client moves to a strictly cheaper Security corp with
/// room (the loser gets `CorpShock::Undercut`), else renews for a week.
fn renew_contracts(world: &mut World) {
    let now = world.tick;
    for seller in world.corps() {
        let Some(due) = world
            .comp::<Corp>(seller)
            .map(|c| c.contracts.iter().filter(|&&(_, t)| t <= now).map(|&(b, _)| b).collect::<Vec<_>>())
        else {
            continue;
        };
        for client in due {
            let mine = contract_price(world, seller);
            let owner = world.owner_of(client);
            let rival = cheapest_seller(world, Some(seller)).filter(|&r| contract_price(world, r) < mine);
            // A Security corp keeps guarding its own buildings itself.
            let rival = rival.filter(|_| owner != Some(seller));
            match rival {
                Some(r) => {
                    end_contract(world, client, &format!("moved to {}", world.owner_label(Some(r))));
                    buy_contract(world, client, r);
                    corp_brain::push_shock(world, seller, CorpShock::Undercut);
                }
                None => {
                    if let Some(c) = world.comp_mut::<Corp>(seller) {
                        for e in c.contracts.iter_mut().filter(|e| e.0 == client) {
                            e.1 = now + CONTRACT_DAYS * TICKS_PER_DAY;
                        }
                    }
                }
            }
        }
    }
}

/// Today's share per corp and niche into `share_hist`; a corp holding
/// Undercut shocks every rival in its niche whose share fell 0.1 in a week.
fn roll_shares(world: &mut World) {
    let corps = world.corps();
    if corps.is_empty() {
        return;
    }
    let all: BTreeMap<Niche, BTreeMap<EntityId, f32>> =
        Niche::ALL.into_iter().map(|n| (n, corp_brain::shares(world, n))).collect();
    for &c in &corps {
        let Some(cc) = world.comp_mut::<Corp>(c) else { continue };
        let niches: Vec<Niche> = cc.niches.iter().copied().collect();
        for n in niches {
            let s = all.get(&n).and_then(|m| m.get(&c)).copied().unwrap_or(0.0);
            let h = cc.share_hist.entry(n).or_default();
            h.push_back(s);
            while h.len() > SHARE_HIST {
                h.pop_front();
            }
        }
    }
    for &c in &corps {
        let Some((CorpOrder::Undercut, Some(n))) = world.comp::<Corp>(c).map(|cc| (cc.order, cc.order_niche)) else {
            continue;
        };
        for r in corp_brain::rivals_in(world, c, n) {
            let fell = world.comp::<Corp>(r).and_then(|rc| rc.share_hist.get(&n)).is_some_and(|h| {
                h.len() >= 2 && h.front().copied().unwrap_or(0.0) - h.back().copied().unwrap_or(0.0) >= 0.1
            });
            if fell {
                corp_brain::push_shock(world, r, CorpShock::Undercut);
            }
        }
    }
}

/// D29: in the red for `bankrupt_days` (and still), or nothing left to own.
fn bankruptcies(world: &mut World) {
    let now = world.tick;
    let limit = world.config.corps.bankrupt_days * TICKS_PER_DAY;
    for c in world.corps() {
        // Still in the red now: a sale since the midnight roll can save it.
        let due = world.comp::<Corp>(c).is_some_and(|cc| {
            cc.buildings.is_empty()
                || (cc.treasury < 0 && cc.negative_since.is_some_and(|t| now.saturating_sub(t) >= limit))
        });
        if due {
            bankrupt(world, c);
        }
    }
}

fn count_monopolies(world: &mut World) {
    let mut n = 0;
    for niche in Niche::ALL {
        let shares = corp_brain::shares(world, niche);
        n += shares
            .iter()
            .filter(|&(&c, &s)| {
                s >= MONOPOLY_SHARE && world.comp::<Corp>(c).is_some_and(|cc| cc.niches.contains(&niche))
            })
            .count() as u32;
    }
    world.stats.current.monopolies = n;
}
