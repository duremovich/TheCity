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

/// The most niches a corp holds (spec § 5: one or two).
pub const MAX_NICHES: usize = 2;

/// D30 plus the Office rule: a building changes hands; a Security Office
/// takes its seller's contracts along when the seller has no other office.
/// A corp buying outside its niches gains the niche (at level 1.0) while it
/// holds fewer than two; a seller left with no building in a niche leaves
/// it (a stale niche kept a rival's price and a third niche in play), and
/// a standing order in it is re-pointed. The new owner staffs the building
/// up to full (phase 5: a bankrupt corp's unpaid Vat Techs had walked out,
/// and nobody re-posted their jobs, so its Farms stopped and Winter starved).
pub fn move_building(world: &mut World, b: EntityId, to: Option<EntityId>) {
    let Some((kind, from)) = world.comp::<Building>(b).map(|bd| (bd.kind, bd.owner)) else { return };
    ownership::transfer_building(world, b, to);
    let niche = corp_brain::niche_of_kind(kind);
    if let (Some(n), Some(t)) = (niche, to) {
        if let Some(c) = world.comp_mut::<Corp>(t) {
            if !c.niches.contains(&n) && c.niches.len() < MAX_NICHES {
                c.niches.insert(n);
                c.price_level.entry(n).or_insert(1.0);
            }
        }
    }
    if let (Some(n), Some(f)) = (niche, from.filter(|&f| world.has::<Corp>(f))) {
        if corp_brain::niche_buildings(world, f, n).is_empty() {
            prune_niche(world, f, n);
        }
    }
    staff_moved(world, b, kind);
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

/// A corp leaves a niche it no longer holds a building in (never its last).
fn prune_niche(world: &mut World, corp: EntityId, n: Niche) {
    let Some(c) = world.comp_mut::<Corp>(corp) else { return };
    if c.niches.len() < 2 || !c.niches.remove(&n) {
        return;
    }
    c.price_level.remove(&n);
    c.share_hist.remove(&n);
    if c.order_niche == Some(n) {
        let first = c.niches.iter().next().copied();
        if matches!(c.order, CorpOrder::Secure | CorpOrder::Hunker | CorpOrder::Lobby) {
            // A corp-wide order is scored in the first niche (D47).
            c.order_niche = first;
        } else {
            if c.order == CorpOrder::Squeeze {
                c.wage_mult = 1.0;
                c.evict_days_override = None;
            }
            c.order = CorpOrder::Hunker;
            c.order_niche = first;
        }
    }
}

/// Top a building that changed hands up to full staff: its open vacancies
/// stay with it (D30) and the new owner posts the rest.
fn staff_moved(world: &mut World, b: EntityId, kind: BuildingKind) {
    let Some(role) = ownership::role_for(kind) else { return };
    let full = corp_brain::full_staff(world, kind);
    let employed = world
        .workers(role)
        .iter()
        .filter(|&&a| world.comp::<crate::components::Job>(a).is_some_and(|j| j.employer == Some(b)))
        .count();
    let open = world.vacancies.get(&b).map_or(0, |v| v.len());
    if employed + open < full {
        world.vacancies.entry(b).or_default().extend(std::iter::repeat_n(role, full - employed - open));
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
/// estate settles and the corp is dissolved. Phase 5 estate rule: a corp
/// buys only in a niche it is already in and holds under `estate_share_cap`
/// of (Nutrix bought 60 Blocks; the richest corp swallowed every estate),
/// and nobody takes more than `estate_buyer_cap` buildings of one estate
/// (one agent bought 41).
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
    // its Farms). Updated as they buy. A deviation from D29's "highest
    // purse", kept on purpose.
    let mut buyers: Vec<(i64, EntityId)> = world
        .corps()
        .into_iter()
        .filter(|&c| c != corp)
        .filter_map(|c| world.comp::<Corp>(c).map(|cc| (cc.treasury - cc.treasury_ref, c)))
        .collect();
    buyers.extend(corp_brain::wallets(world));
    let cfg = world.config.corps.clone();
    let shares: BTreeMap<Niche, BTreeMap<EntityId, f32>> =
        Niche::ALL.into_iter().map(|n| (n, corp_brain::shares(world, n))).collect();
    let mut taken: BTreeMap<EntityId, usize> = BTreeMap::new();
    for b in buildings {
        let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { continue };
        let v = ownership::value(world, kind);
        let niche = corp_brain::niche_of_kind(kind);
        let barred = |id: EntityId| {
            if cfg.estate_buyer_cap > 0 && taken.get(&id).is_some_and(|&k| k >= cfg.estate_buyer_cap) {
                return true;
            }
            let Some(c) = world.comp::<Corp>(id) else { return false };
            let Some(n) = niche else { return false };
            (cfg.estate_niche_only && !c.niches.contains(&n))
                || shares.get(&n).and_then(|m| m.get(&id)).is_some_and(|&s| s >= cfg.estate_share_cap)
        };
        let pick = if v > 0 {
            buyers
                .iter()
                .enumerate()
                .filter(|(_, &(p, id))| p >= v && !barred(id))
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
                *taken.entry(buyer).or_default() += 1;
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
/// to the city. Why the Treasury, uncapped: a corp goes below zero only
/// through `charge`, whose payee is the Treasury (upkeep, restock): the
/// overdraft is coins the city was credited and never received, so writing
/// it back conserves money (the corp's debt was to the city).
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
    // Split each kind (every second one, from the second), so the spinoff
    // takes half of what carries the share: Markets for Food (a lone Market
    // stayed with the parent when the Farms sorted first), Offices for
    // Security (and half the contracts with them: moved Offices took none),
    // Blocks for Housing.
    let carrier = match niche {
        Niche::Food => BuildingKind::Market,
        Niche::Housing => BuildingKind::Home,
        Niche::Security => BuildingKind::SecurityOffice,
    };
    let all = corp_brain::niche_buildings(world, corp, niche);
    let of_kind = |k: BuildingKind| -> Vec<EntityId> {
        all.iter().copied().filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == k)).collect()
    };
    if of_kind(carrier).len() < 2 {
        return Err(format!("{name} has one {}; a split cannot break the {niche} monopoly", carrier.label()));
    }
    let mut moved: Vec<EntityId> = Vec::new();
    for &k in corp_brain::niche_kinds(niche) {
        moved.extend(of_kind(k).into_iter().skip(1).step_by(2));
    }
    moved.sort();
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
    if niche == Niche::Security {
        // Every second contract (by client) follows the Offices, up to room.
        let contracts = world.comp::<Corp>(corp).map(|c| c.contracts.clone()).unwrap_or_default();
        let room = capacity(world, spinoff);
        let shift: Vec<(EntityId, crate::time::Tick)> = contracts
            .into_iter()
            .filter(|&(client, _)| world.owner_of(client) != Some(corp))
            .skip(1)
            .step_by(2)
            .take(room)
            .collect();
        for &(client, until) in &shift {
            if let Some(c) = world.comp_mut::<Corp>(corp) {
                c.contracts.retain(|&(b, _)| b != client);
            }
            if let Some(s) = world.comp_mut::<Corp>(spinoff) {
                if let Err(i) = s.contracts.binary_search_by_key(&client, |&(b, _)| b) {
                    s.contracts.insert(i, (client, until));
                }
            }
            if let Some(cb) = world.comp_mut::<Building>(client) {
                cb.secured_by = Some(spinoff);
            }
        }
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
            // Only the day the week's drop first reaches 0.1: pushed every
            // day it lasted, a 0.3 shock was always pending and any Robbed
            // shock forced a rescore with no hysteresis.
            let fell = world.comp::<Corp>(r).and_then(|rc| rc.share_hist.get(&n)).is_some_and(|h| {
                let len = h.len();
                let front = h.front().copied().unwrap_or(0.0);
                let now = h.back().copied().unwrap_or(0.0);
                let before = if len >= 3 { h[len - 2] } else { front };
                len >= 2 && front - now >= 0.1 && front - before < 0.1
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
