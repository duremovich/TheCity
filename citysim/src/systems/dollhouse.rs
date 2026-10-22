//! Jobs & lives phase 5, the chip contract (docs/JOBS_AND_LIVES.md § 5.2,
//! D8, D9). Under `[vice] chip` (itself under `[vice] enabled` and the asset
//! store, `[assets] enabled`, which chrome needs): off, every entry point is
//! a no-op, no Dollhouse stands and no state is written.
//!
//! A game mechanic in numbers only: a labour contract with a timed implant.
//!
//! - **The venue.** A Dollhouse is a venue on the leisure ladder's high
//!   rung, open while a contracted worker is on shift. Its posts (the `chip`
//!   trade row) are never filled by the hiring pass: only by a contract.
//! - **The contract.** At midnight a Dollhouse's open places are offered to
//!   jobless debtors (owed coins on their edges); one takes it when its coins
//!   are below its debt (§ 12's probe), ranked by dress (ties the lower id). The
//!   hire installs a `Slot::Behaviour` implant through the chrome path
//!   (`chrome::install`, the holder paying `chip_fee` to the nearest
//!   Clinic), at the row's wage × `chip_mult` (`Job.premium`), for
//!   `chip_days`. The corp contract is legal (`dollhouse_contracts`).
//! - **Under the chip.** While a contracted worker is on shift no
//!   `MemoryEntry` is written for it (`World::remember`) and its mood is not
//!   updated (`mood`); the implant's `chip_load` rides `Kit.load` into
//!   `chrome::sanity_daily` and its episodes, unchanged. An episode ends the
//!   contract early.
//! - **The end.** At `until` (or early) the implant comes out (the holder
//!   pays `chip_fee` again; the asset is gone), the Job ends, and the
//!   `Chipped` trait stays: a one-time sanity cut of `chipped_sanity` and a
//!   `Lost` memory of the holder.
//! - **Captives.** A gang's Dollhouse front holds captives through the M16b
//!   machinery (a gang order to abduct, the hold at the front, a ransom put
//!   to the captive's kin). A captive held there is chipped on arrival and,
//!   with `captive_chip_trafficking` on, the law gets `Trafficking` on the
//!   member who handed it over. Its chip comes out when the hold ends. A
//!   corp's Dollhouse never holds one.
//!
//! Every coin moves between purses. The hourly pass is O(contracts); the
//! midnight pass O(adults) once a day (the offer's debtors).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{
    Asset, AssetKind, AssetLoc, Brain, Building, BuildingKind, Chipped, Corp, Gang, Job, Lod, MemoryKind, Position,
    Role, Sentence, Slot, Wallet,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::world::World;

/// The chip is live: `[vice] enabled`, `[vice] chip` and the asset store.
pub fn on(world: &World) -> bool {
    crate::systems::vice::on(world) && world.config.vice.chip && world.config.assets.enabled
}

/// One contract (a worker's, or a captive's at a gang's holding).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChipContract {
    /// The Dollhouse.
    pub at: EntityId,
    /// Who pays the fees and owns the implant (the Dollhouse's owner, or
    /// the holding gang).
    pub holder: Option<EntityId>,
    pub since: Tick,
    pub until: Tick,
    /// The implant.
    pub chip: EntityId,
    pub captive: bool,
}

/// Phase 5's chip state on `Jobs2State` (empty, and not saved, off).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChipState {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub contracts: BTreeMap<EntityId, ChipContract>,
}

impl ChipState {
    pub fn is_default(&self) -> bool {
        *self == ChipState::default()
    }
}

/// The `chip` trade row's role (the first live one).
pub fn chip_role(world: &World) -> Option<Role> {
    let row = world.config.trades.iter().find(|r| r.chip && crate::systems::classpay::trade_live(world, r))?;
    world.config.trade_role(&row.id)
}

/// A `chip` row's posts are never filled by the hiring pass.
pub fn hire_ok(world: &World, role: Role) -> bool {
    match role {
        Role::Trade(t) => !world.config.trade(t).is_some_and(|r| r.chip),
        _ => true,
    }
}

/// Is `id` under its chip now: a captive's for the whole hold, a worker's
/// while on shift (a cheap read off the contracts map; false off).
pub fn under_chip(world: &World, id: EntityId) -> bool {
    let Some(c) = world.jobs2.chip.contracts.get(&id) else { return false };
    if c.captive {
        return true;
    }
    let tod = world.tick_of_day();
    world.comp::<Job>(id).is_some_and(|j| j.employer == Some(c.at) && j.on_shift(tod))
}

/// The coins `id` owes on its edges (the lower id owes when `debt > 0`).
pub fn debt_of(world: &World, id: EntityId) -> i64 {
    world
        .neighbours(id)
        .filter_map(|o| {
            let e = world.edge(id, o)?;
            let (lo, _) = crate::components::edge_key(id, o);
            let owed = if lo == id { e.debt } else { -e.debt };
            (owed > 0).then_some(i64::from(owed))
        })
        .sum()
}

/// § 12's probe: a debtor offered the contract takes it when its coins are
/// below its debt (a paid debt refuses).
pub fn takes_offer(world: &World, id: EntityId) -> bool {
    let debt = debt_of(world, id);
    debt > 0 && world.comp::<Wallet>(id).map_or(0, |w| w.coins) < debt
}

/// A contracted worker of `b` on shift now, inside it (a Statistical one
/// counts as there): the Dollhouse is open to visitors.
pub fn staffed_now(world: &World, b: EntityId) -> bool {
    let tod = world.tick_of_day();
    world.jobs2.chip.contracts.iter().any(|(&id, c)| {
        c.at == b
            && !c.captive
            && world.comp::<Job>(id).is_some_and(|j| j.employer == Some(b) && j.on_shift(tod))
            && world.comp::<Brain>(id).is_some_and(|br| {
                br.lod == Lod::Statistical || world.comp::<Position>(id).is_some_and(|p| p.building == Some(b))
            })
    })
}

/// A visit's price at a Dollhouse by its tier.
pub fn visit_price(world: &World, tier: u8) -> i64 {
    world.config.vice.dollhouse_price[usize::from(tier.min(2))]
}

/// A paid Dollhouse visit done (the ladder's `Enjoy`): counted.
pub fn on_visit(world: &mut World, b: EntityId, price: i64) {
    if !on(world) || world.comp::<Building>(b).is_none_or(|bd| bd.kind != BuildingKind::Dollhouse) {
        return;
    }
    let row = ownership::jobs2_row(world);
    row.dollhouse_visits += 1;
    row.vice_revenue += price;
}

fn standing(world: &World, b: EntityId) -> bool {
    world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
}

/// The standing Clinic nearest `b`'s door (ties the lower id).
fn nearest_clinic(world: &World, b: EntityId) -> Option<EntityId> {
    let from = world.comp::<Building>(b)?.door;
    world
        .buildings_of_kind(BuildingKind::Clinic)
        .iter()
        .copied()
        .filter(|&c| standing(world, c))
        .filter_map(|c| world.comp::<Building>(c).map(|bd| (bd.door.manhattan(from), c)))
        .min()
        .map(|(_, c)| c)
}

/// The holder pays a Clinic `chip_fee` (what it can; `Flow::Asset`).
fn pay_fee(world: &mut World, holder: Option<EntityId>, clinic: EntityId) {
    let fee = world.config.vice.chip_fee.min(world.purse(holder).max(0));
    if fee > 0 {
        let to = world.owner_of(clinic);
        let paid = ownership::pay(world, holder, to, fee, Flow::Asset);
        ownership::credit(world, clinic, paid);
    }
}

/// Install a behaviour implant in `id` through the chrome path at the Clinic
/// nearest `at` (the holder pays the fee and keeps the implant). `None`
/// with no Clinic.
fn install_chip(world: &mut World, id: EntityId, holder: Option<EntityId>, at: EntityId) -> Option<EntityId> {
    // Jobs & lives phase 6 (§ 7): a gang holder's own BlackClinic first (the
    // black install; nothing with `[gray]` off).
    let black = holder
        .filter(|&h| world.has::<Gang>(h) && crate::systems::gray::on(world))
        .and_then(|h| crate::systems::gray::black_clinic_of(world, h));
    let clinic = black.or_else(|| nearest_clinic(world, at))?;
    pay_fee(world, holder, clinic);
    let kind = AssetKind::Implant(Slot::Behaviour);
    let list = crate::systems::assets::list_price(world, kind, 1).unwrap_or(0);
    let a = crate::systems::assets::spawn_asset(world, kind, 1, holder, AssetLoc::Stock(clinic), list);
    if crate::systems::chrome::install(world, id, clinic, a, 0).is_err() {
        crate::systems::assets::despawn(world, a);
        return None;
    }
    crate::systems::assets::set_owner(world, a, holder);
    Some(a)
}

/// Begin a contract for `id` at `at` (a hire, or a captive's).
fn begin(world: &mut World, id: EntityId, at: EntityId, holder: Option<EntityId>, captive: bool) -> bool {
    let Some(chip) = install_chip(world, id, holder, at) else { return false };
    let now = world.tick;
    let until = now + Tick::from(world.config.vice.chip_days.max(1)) * TICKS_PER_DAY;
    world.jobs2.chip.contracts.insert(id, ChipContract { at, holder, since: now, until, chip, captive });
    let row = ownership::jobs2_row(world);
    if captive {
        row.captives_chipped += 1;
    } else {
        row.chip_hired += 1;
    }
    let text = format!(
        "{} began a {}-day chip contract at {}",
        world.name_of(id),
        world.config.vice.chip_days,
        world.name_of(at)
    );
    world.push_event(EventKind::Hire, &[id, at], text);
    true
}

/// End `id`'s contract (`why`): the implant out (the holder pays the fee),
/// the post vacated, the `Chipped` trait, the sanity cut, the memory.
fn end(world: &mut World, id: EntityId, why: &str) {
    let Some(c) = world.jobs2.chip.contracts.remove(&id) else { return };
    let living = crate::systems::law::living(world, id);
    let installed = world.comp::<Asset>(c.chip).is_some_and(|x| x.loc == AssetLoc::Installed(id));
    if installed {
        if let Some(clinic) = nearest_clinic(world, c.at) {
            pay_fee(world, c.holder, clinic);
        }
        crate::systems::assets::despawn(world, c.chip);
    }
    if world.comp::<Job>(id).is_some_and(|j| j.employer == Some(c.at)) {
        let role = world.comp::<Job>(id).map(|j| j.role);
        crate::systems::economy::quit_job(world, id, "contract over");
        if let Some(r) = role {
            crate::systems::economy::withdraw_vacancy(world, c.at, r);
        }
    }
    let row = ownership::jobs2_row(world);
    row.chip_ended += 1;
    if why == "episode" {
        row.chip_early += 1;
    }
    if !living {
        return;
    }
    let days = (world.tick.saturating_sub(c.since) / TICKS_PER_DAY) as u32;
    world.insert(id, Chipped { holder: c.holder, days, captive: c.captive });
    let cut = world.config.vice.chipped_sanity;
    if let Some(b) = world.comp_mut::<crate::components::Body>(id) {
        b.sanity = (b.sanity - cut).clamp(0.0, 1.0);
    }
    world.remember(id, MemoryKind::Lost, c.holder, 0.5, -0.5, false);
    let text = format!("{}'s chip contract at {} ended ({why}, {days} days)", world.name_of(id), world.name_of(c.at));
    world.push_event(EventKind::Quit, &[id, c.at], text);
}

// ---------------------------------------------------------------------------
// The passes
// ---------------------------------------------------------------------------

/// Hourly (from `living::run`): a contracted worker in an episode ends its
/// contract early (counted at the venue). O(contracts).
pub fn hourly(world: &mut World) {
    if !on(world) || world.jobs2.chip.contracts.is_empty() || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let ids: Vec<EntityId> = world.jobs2.chip.contracts.keys().copied().collect();
    for id in ids {
        if crate::systems::chrome::in_episode(world, id) {
            ownership::jobs2_row(world).chip_episodes += 1;
            end(world, id, "episode");
        }
    }
}

/// Midnight (from `living::run`): contracts past their end (or lost: dead,
/// fired, the implant gone, the hold over) end; the captives at a gang's
/// Dollhouse are chipped; the open places are offered; the fronts' orders.
pub fn daily(world: &mut World) {
    if !on(world) {
        return;
    }
    let _t = crate::prof::scope!("dollhouse::daily");
    close_contracts(world);
    chip_captives(world);
    offer_places(world);
    front_orders(world);
    tidy_vacancies(world);
}

fn close_contracts(world: &mut World) {
    let now = world.tick;
    let all: Vec<(EntityId, ChipContract)> =
        world.jobs2.chip.contracts.iter().map(|(&id, c)| (id, c.clone())).collect();
    for (id, c) in all {
        let lost = !crate::systems::law::living(world, id)
            || world.comp::<Asset>(c.chip).is_none_or(|x| x.loc != AssetLoc::Installed(id));
        let gone = if c.captive {
            world.captives.get(&id).is_none_or(|k| k.held_at != c.at)
        } else {
            world.comp::<Job>(id).is_none_or(|j| j.employer != Some(c.at))
        };
        if lost || gone {
            end(world, id, "cut short");
        } else if now >= c.until && !c.captive {
            // Phase-8 review: a captive's chip stays in while it is held (its
            // end is the hold's end, `gone` above); ending it at `chip_days`
            // re-chipped the same captive in this midnight pass, the fee,
            // the Trafficking report and the sanity cut repeating.
            end(world, id, "served");
        }
    }
}

/// A captive held at a Dollhouse its gang holds, not yet chipped: chipped,
/// and `Trafficking` filed on the member who handed it over.
fn chip_captives(world: &mut World) {
    let held: Vec<(EntityId, EntityId, Option<EntityId>, EntityId)> = world
        .captives
        .iter()
        .filter(|(id, k)| {
            !world.jobs2.chip.contracts.contains_key(id)
                && world.comp::<Building>(k.held_at).is_some_and(|b| b.kind == BuildingKind::Dollhouse)
                && k.holder.is_some_and(|h| world.has::<Gang>(h))
        })
        .map(|(&id, k)| (id, k.held_at, k.holder, k.agent))
        .collect();
    for (id, at, holder, agent) in held {
        if !begin(world, id, at, holder, true) {
            continue;
        }
        if world.levers.vice.captive_chip_trafficking && crate::systems::law::living(world, agent) {
            crate::systems::law::file_report(world, crate::components::Crime::Trafficking, agent, None);
            ownership::jobs2_row(world).trafficking += 1;
        }
    }
}

/// A Dollhouse's places: the chip row's `staff_per_floor` × its floors.
fn places(world: &World, b: EntityId, role: Role) -> usize {
    let per = match role {
        Role::Trade(t) => world.config.trade(t).map_or(0, |r| usize::from(r.staff_per_floor)),
        _ => 0,
    };
    per * usize::from(world.comp::<Building>(b).map_or(1, |bd| bd.floors.max(1)))
}

/// The open places at each standing Dollhouse (ascending) offered to the
/// debtors who take it (coins below the debt), by dress then id. A corp's
/// only with `dollhouse_contracts` on; the holder must cover the fee.
fn offer_places(world: &mut World) {
    let Some(role) = chip_role(world) else { return };
    let houses: Vec<EntityId> =
        world.buildings_of_kind(BuildingKind::Dollhouse).iter().copied().filter(|&b| standing(world, b)).collect();
    if houses.is_empty() {
        return;
    }
    let mut open: Vec<(EntityId, usize)> = Vec::new();
    for b in houses {
        let holder = world.owner_of(b);
        if holder.is_some_and(|o| world.has::<Corp>(o)) && !world.levers.vice.dollhouse_contracts {
            continue;
        }
        let have = world.jobs2.chip.contracts.values().filter(|c| c.at == b && !c.captive).count();
        let n = places(world, b, role).saturating_sub(have);
        if n > 0 && world.purse(holder) >= world.config.vice.chip_fee {
            open.push((b, n));
        }
    }
    if open.is_empty() {
        return;
    }
    let execs = crate::systems::classes::exec_set(world);
    let mut pool: Vec<(std::cmp::Reverse<u8>, EntityId)> = world
        .citizens()
        .into_iter()
        .filter(|&id| {
            crate::systems::demography::is_adult(world, id)
                && !world.has::<Sentence>(id)
                && !execs.contains(&id)
                && !world.jobs2.chip.contracts.contains_key(&id)
                && !crate::systems::captives::is_captive(world, id)
                && world.comp::<Brain>(id).is_some_and(|b| !b.emigrating && b.cuffed_by.is_none())
                // § 5.2's hire pool: the desperate, without a post.
                && !world.has::<Job>(id)
                && takes_offer(world, id)
        })
        .map(|id| (std::cmp::Reverse(crate::systems::reputation::appearance_of(world, id).dress), id))
        .collect();
    pool.sort_unstable();
    let mut left: Vec<EntityId> = pool.into_iter().map(|(_, id)| id).collect();
    for (b, n) in open {
        for _ in 0..n {
            // The island pass phase 1 (OPEN-163): in the pool's order, a
            // candidate past `[distance] job_cap_ticks` from the post is
            // passed over while one within it is left, else the nearest
            // (off: the pool's first).
            let door = world.comp::<crate::components::Building>(b).map(|x| x.door);
            let at = match (world.config.distance.as_ref(), door) {
                (Some(cfg), Some(door)) => crate::systems::distance::pick(
                    left.iter().enumerate().map(|(i, &id)| {
                        let t = crate::systems::distance::set_out(world, id)
                            .map_or(crate::time::Tick::MAX, |from| crate::systems::distance::travel(world, from, door));
                        (t, i as i64, i)
                    }),
                    cfg.job_cap_ticks,
                ),
                _ => (!left.is_empty()).then_some(0),
            };
            let Some(at) = at else { return };
            let id = left.remove(at);
            crate::systems::demography::hire(world, id, b, role);
            let mult = world.config.vice.chip_mult;
            if let Some(j) = world.comp_mut::<Job>(id) {
                j.premium = mult;
            }
            let holder = world.owner_of(b);
            if !begin(world, id, b, holder, false) {
                crate::systems::economy::quit_job(world, id, "no clinic");
                crate::systems::economy::withdraw_vacancy(world, b, role);
            }
        }
    }
}

/// A gang's Dollhouse front with room under `captives_per_front` orders the
/// abduction of a homeless adult in a district it holds (the lowest id; a
/// ransom put to its kin), through the M16b machinery. One order at a time.
fn front_orders(world: &mut World) {
    if !crate::systems::captives::on(world) {
        return;
    }
    let cap = world.config.vice.captives_per_front as usize;
    for g in world.gangs() {
        let fronts: Vec<EntityId> = crate::systems::leisure::fronts_of(world, g)
            .into_iter()
            .filter(|&b| world.comp::<Building>(b).is_some_and(|bd| bd.kind == BuildingKind::Dollhouse))
            .collect();
        let Some(&at) = fronts.first() else { continue };
        if crate::systems::captives::held_at(world, at) >= cap || crate::systems::captives::abducting(world, g) {
            continue;
        }
        let held: Vec<crate::components::DistrictId> =
            crate::systems::gang::held_districts(world, g).into_iter().map(|(d, _)| d).collect();
        let victim = held
            .iter()
            .flat_map(|&d| world.districts.get(d.index()).map(|x| x.residents.clone()).unwrap_or_default())
            .filter(|&v| {
                crate::systems::demography::is_adult(world, v)
                    && crate::systems::law::living(world, v)
                    && !world.has::<Sentence>(v)
                    && world.gang_of(v).is_none()
                    && !crate::systems::captives::is_captive(world, v)
                    && world.comp::<crate::components::Household>(v).is_none_or(|h| h.home.is_none())
            })
            .min();
        if let Some(v) = victim {
            crate::systems::captives::front_abduct(world, g, v, at);
        }
    }
}

/// The hiring pass never fills a `chip` post: any vacancy for it is dropped.
fn tidy_vacancies(world: &mut World) {
    let Some(role) = chip_role(world) else { return };
    for b in world.buildings_of_kind(BuildingKind::Dollhouse).to_vec() {
        if let Some(v) = world.vacancies.get_mut(&b) {
            v.retain(|&r| r != role);
            if v.is_empty() {
                world.vacancies.remove(&b);
            }
        }
    }
}

/// `World::new`, after the vice's venues: `dollhouses` Dollhouses per copy
/// of each `[vice] corp_rows` row on the Lots left. No RNG, no coins.
pub fn seed(world: &mut World) {
    if !on(world) {
        return;
    }
    let n = world.config.vice.dollhouses;
    let rows = world.config.vice.corp_rows.clone();
    for j in 0..world.seed_copies() {
        for &r in &rows {
            let Some(t) = world.config.corps.row_template(r) else { continue };
            let Some(&c) = world.corps_of_template(t).get(j) else { continue };
            for _ in 0..n {
                crate::systems::vice::seed_venue(world, BuildingKind::Dollhouse, Some(c));
            }
        }
    }
}

/// The day's end (`systems::stats`): the chip's snapshots.
pub fn snapshot(world: &mut World) {
    if !on(world) {
        return;
    }
    let houses = world.buildings_of_kind(BuildingKind::Dollhouse).iter().filter(|&&b| standing(world, b)).count();
    let contracts = world.jobs2.chip.contracts.len() as u32;
    let chipped = world.citizens().into_iter().filter(|&id| world.has::<Chipped>(id)).count() as u32;
    let row = ownership::jobs2_row(world);
    row.dollhouses = houses as u32;
    row.chip_contracts = contracts;
    row.chipped_living = chipped;
}
