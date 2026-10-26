//! The pressure pass phase 6 (docs/PRESSURE_PASS.md § Phase 6): a business
//! under water. A person-owned business whose till cannot meet its wages
//! paid short until the staff walked out (F3: 152 unpaid walkouts at bars in
//! 30 days at 20k). Under `[underwater]` it is a story with choices, made by
//! the owner once a day at midnight (both tiers), weighted by his
//! personality, his misery and what is on offer:
//!
//! - **Hang on** (`w_hold × (0.25 + pride)`): nothing changes; he bleeds.
//! - **Shorter hours** (`w_hours × (0.5 + loyalty)`, once): every hand's
//!   shift and wage cut by `hours_cut`; a hiring freeze while short.
//! - **Lay staff off** (`w_layoff × (1.5 − loyalty)`): `layoff_share` of the
//!   staff, newest first, `LaidOff` naming the till; wages owed stay a debt
//!   and a stiffed worker (below); a hiring freeze while short.
//! - **Borrow** (`w_borrow × (0.5 + courage) × (1 − misery / 2)`, one loan at
//!   a time): `loan_days` of the bill from a gang holding the district, a
//!   loan shark (a lawless rich adult of the district) or a corp (the lawful
//!   only), repaid weekly with the lender's rate. A missed installment sends
//!   the gang's toughest member or the shark in person (a `Visit`): paid, or
//!   what there is taken and a beating; at `seize_misses` the gang or shark
//!   takes the business (a gang's becomes a front); a corp repossesses.
//! - **Sell** (`w_sell × (0.5 + greed) × escalation`): a fire sale to a corp
//!   in the niche, or to the district's gang as a front (the lawless, or the
//!   desperate past `front_misery`). A bought front's staff join the gang
//!   over the days, the gang books part of its week's take through the till
//!   (laundered) and a rival holding turf in the district hits it.
//! - **Close** (`w_close × (0.25 + misery) × escalation`): staff let go, the
//!   doors shut, the building for sale to the next founder (a would-be
//!   founder with the coins buys and reopens it); a Bar or Hotel unsold for
//!   `shut_derelict_days` goes derelict.
//!
//! **The stiffed worker** (an unpaid walkout, F3, or a layoff owed wages)
//! comes back: on `confront_p × (0.5 + pride + misery)` a day he walks to the
//! owner in person (a `Visit`, `VisitWhy::Confront`; a Statistical worker is
//! credited at the hour) and demands the wages. The owner pays (what he
//! holds) on his lawfulness, loyalty and greed; refused, the grudge grows,
//! and past `fight_grudge` the worker swings; a lawful worker reports him to
//! the Hall instead, which then takes the owed out of the owner's purse as
//! it comes in.
//!
//! Statistical mirror: the decisions, the loans, the sales and closures are
//! midnight passes, tier-blind; the confrontation and the collector's call
//! are `Visit`s, which a Statistical visitor is credited at its hour.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::{
    edge_key, Brain, Building, BuildingKind, Controller, Corp, Crime, Gang, GangMember, Identity, Job, Personality,
    Position, Sentence, Wallet,
};
use crate::config::UnderwaterCfg;
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::household::{PendingVisit, VisitWhy};
use crate::systems::ownership::{self, Flow};
use crate::time::TICKS_PER_DAY;
use crate::world::World;

const PURPOSE_CHOICE: u64 = 0x0b17_0001;
const PURPOSE_LENDER: u64 = 0x0b17_0002;
const PURPOSE_CONFRONT: u64 = 0x0b17_0003;
const PURPOSE_PAY: u64 = 0x0b17_0004;
const PURPOSE_HOUR: u64 = 0x0b17_0005;
const PURPOSE_JOIN: u64 = 0x0b17_0006;
const PURPOSE_HIT: u64 = 0x0b17_0007;
const PURPOSE_BUYER: u64 = 0x0b17_0008;

/// Who lent the money.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lender {
    Gang,
    Shark,
    Corp,
}

impl Lender {
    fn label(self) -> &'static str {
        match self {
            Lender::Gang => "the gang",
            Lender::Shark => "a loan shark",
            Lender::Corp => "a corp",
        }
    }
}

/// A business's run under water.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Biz {
    /// Consecutive short midnights.
    pub short_days: u16,
    /// The day of the last decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_day: Option<u64>,
    /// Decisions made this run (sell and close escalate with it).
    pub rounds: u8,
    /// A hiring freeze (after a cut).
    pub freeze: bool,
    pub hours_cut: bool,
}

/// A loan against a business.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loan {
    pub building: EntityId,
    pub borrower: EntityId,
    /// The gang, the shark (a person) or the corp.
    pub lender: EntityId,
    pub kind: Lender,
    pub principal: i64,
    /// Still owed.
    pub left: i64,
    pub installment: i64,
    /// The day the next installment falls due.
    pub next_due: u64,
    pub missed: u8,
}

/// A worker owed wages by a person (the edge debt holds the coins).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stiff {
    pub worker: EntityId,
    pub owner: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<EntityId>,
    pub since: u64,
    pub tries: u8,
    /// Reported to the Hall: the owed is garnished as it comes in.
    pub reported: bool,
}

/// A shut business, for sale.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shut {
    pub since: u64,
    pub price: i64,
}

/// `World.underwater`: saved while it holds anything.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub biz: BTreeMap<EntityId, Biz>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loans: Vec<Loan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stiffs: Vec<Stiff>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shut: BTreeMap<EntityId, Shut>,
    /// Fronts a gang bought or seized: building -> gang.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fronts: BTreeMap<EntityId, EntityId>,
}

impl State {
    pub fn is_empty(&self) -> bool {
        self.biz.is_empty()
            && self.loans.is_empty()
            && self.stiffs.is_empty()
            && self.shut.is_empty()
            && self.fronts.is_empty()
    }
}

pub fn cfg(world: &World) -> Option<&UnderwaterCfg> {
    world.config.underwater.as_ref()
}

fn hash(world: &World, purpose: u64, a: EntityId, b: u64) -> f32 {
    crate::systems::econ::hash_unit(world.seed(), purpose, u64::from(a.index), b)
}

#[derive(Clone, Copy)]
struct Traits {
    lawful: f32,
    greed: f32,
    pride: f32,
    courage: f32,
    loyalty: f32,
}

fn traits(world: &World, id: EntityId) -> Traits {
    world.comp::<Personality>(id).map_or(
        Traits { lawful: 0.5, greed: 0.5, pride: 0.5, courage: 0.5, loyalty: 0.5 },
        |p| Traits { lawful: p.lawfulness, greed: p.greed, pride: p.pride, courage: p.courage, loyalty: p.loyalty },
    )
}

fn living(world: &World, id: EntityId) -> bool {
    crate::systems::law::living(world, id)
}

fn person(world: &World, id: EntityId) -> bool {
    world.has::<Identity>(id) && world.has::<Brain>(id) && living(world, id)
}

fn name(world: &World, id: EntityId) -> String {
    match world.comp::<Gang>(id) {
        Some(g) => g.name.clone(),
        None => match world.comp::<Corp>(id) {
            Some(c) => c.name.clone(),
            None => world.name_of(id),
        },
    }
}

fn household(world: &World, owner: EntityId, who: EntityId) -> bool {
    who == owner || world.spouse_of(owner) == Some(who)
}

/// `b`'s staff outside the owner's household and their day's wages; the
/// most days any of them is owed.
fn payroll(world: &World, b: EntityId, owner: EntityId) -> (i64, Vec<EntityId>, u8) {
    let mut bill = 0;
    let mut staff = Vec::new();
    let mut unpaid = 0u8;
    for &s in world.staff_of(b) {
        if household(world, owner, s) {
            continue;
        }
        let Some(j) = world.comp::<Job>(s).filter(|j| j.employer == Some(b)) else { continue };
        bill += j.wage_per_day.max(0);
        unpaid = unpaid.max(j.days_unpaid);
        staff.push(s);
    }
    (bill, staff, unpaid)
}

/// The kinds that never count as a business here.
fn exempt(kind: BuildingKind) -> bool {
    matches!(
        kind,
        BuildingKind::Home
            | BuildingKind::Lot
            | BuildingKind::Hideout
            | BuildingKind::Camp
            | BuildingKind::Mission
            | BuildingKind::Prison
            | BuildingKind::Station
    )
}

/// Person-owned standing businesses with an owner alive, ascending.
fn businesses(world: &World) -> Vec<(EntityId, EntityId)> {
    let mut out = Vec::new();
    for kind in BuildingKind::ALL {
        if exempt(kind) {
            continue;
        }
        for &b in world.buildings_of_kind(kind) {
            let Some(bd) = world.comp::<Building>(b) else { continue };
            if bd.demolished || bd.derelict {
                continue;
            }
            let Some(o) = bd.owner.filter(|&o| person(world, o)) else { continue };
            out.push((b, o));
        }
    }
    out.sort_unstable();
    out
}

/// Owners of person-owned businesses with staff to pay, by the till over a
/// week's wage bill, lowest first (the shadow's `owner_broke` pick).
pub fn owners_by_till(world: &World) -> Vec<(f32, EntityId)> {
    let mut v: Vec<(f32, EntityId)> = businesses(world)
        .into_iter()
        .filter_map(|(b, o)| {
            let (bill, staff, _) = payroll(world, b, o);
            (!staff.is_empty() && bill > 0).then(|| (world.purse(Some(o)) as f32 / (7 * bill) as f32, o))
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    v.dedup_by_key(|x| x.1);
    v
}

// ---------------------------------------------------------------------------
// Debts between people (the edge's `debt`: the lower id owes the higher when > 0)
// ---------------------------------------------------------------------------

/// What `debtor` owes `creditor` on their edge.
pub fn owed(world: &World, debtor: EntityId, creditor: EntityId) -> i64 {
    let Some(e) = world.edge(debtor, creditor) else { return 0 };
    let (lo, _) = edge_key(debtor, creditor);
    i64::from(if lo == debtor { e.debt } else { -e.debt }).max(0)
}

pub(crate) fn owe(world: &mut World, debtor: EntityId, creditor: EntityId, amount: i64) {
    if amount <= 0 {
        return;
    }
    let (lo, _) = edge_key(debtor, creditor);
    let add = i32::try_from(amount).unwrap_or(i32::MAX);
    let tick = world.tick;
    let e = world.edge_entry(debtor, creditor);
    e.debt = e.debt.saturating_add(if lo == debtor { add } else { -add });
    e.debt_since.get_or_insert(tick);
}

fn settle(world: &mut World, debtor: EntityId, creditor: EntityId, paid: i64) {
    if paid <= 0 || world.edge(debtor, creditor).is_none() {
        return;
    }
    let (lo, _) = edge_key(debtor, creditor);
    let sub = i32::try_from(paid).unwrap_or(i32::MAX);
    let e = world.edge_entry(debtor, creditor);
    if lo == debtor {
        e.debt = (e.debt - sub).max(0);
    } else {
        e.debt = (e.debt + sub).min(0);
    }
    if e.debt == 0 {
        e.debt_since = None;
    }
}

/// F3's unpaid walkout (or a layoff owed wages): the worker is owed by a
/// person and may come back for it (`economy::walk_out_unpaid`).
pub fn note_stiffed(world: &mut World, worker: EntityId, owner: EntityId, place: Option<EntityId>) {
    if cfg(world).is_none() || worker == owner {
        return;
    }
    let today = world.day();
    let s = &mut world.underwater.stiffs;
    if let Some(x) = s.iter_mut().find(|x| x.worker == worker && x.owner == owner) {
        x.place = place.or(x.place);
        return;
    }
    s.push(Stiff { worker, owner, place, since: today, tries: 0, reported: false });
}

// ---------------------------------------------------------------------------
// Midnight
// ---------------------------------------------------------------------------

/// Midnight (LivingEarly, after the jobs' top-up and the visits due): the
/// loans' installments, the stiffed workers, the shut businesses, the
/// fronts, then every person-owned business's day and its owner's choice;
/// the freezes last (before `demography` hires).
pub fn daily(world: &mut World) {
    let Some(c) = cfg(world).cloned() else { return };
    loans_daily(world, &c);
    stiffs_daily(world, &c);
    shut_daily(world, &c);
    fronts_daily(world, &c);
    scan(world, &c);
    let frozen: Vec<EntityId> = world
        .underwater
        .biz
        .iter()
        .filter(|(_, x)| x.freeze)
        .map(|(&b, _)| b)
        .chain(world.underwater.shut.keys().copied())
        .collect();
    for b in frozen {
        world.vacancies.remove(&b);
    }
}

fn scan(world: &mut World, c: &UnderwaterCfg) {
    let today = world.day();
    let mut seen: Vec<EntityId> = Vec::new();
    for (b, owner) in businesses(world) {
        if world.underwater.shut.contains_key(&b) {
            continue;
        }
        let (bill, staff, unpaid) = payroll(world, b, owner);
        if staff.is_empty() || bill <= 0 {
            // Every hand let go: the freeze holds until the purse would
            // carry the kind's staff again.
            let hint = payroll_hint(world, b, owner);
            if world.underwater.biz.get(&b).is_some_and(|x| x.freeze)
                && (world.purse(Some(owner)) as f32) < c.recover_days * hint as f32
            {
                seen.push(b);
            }
            continue;
        }
        seen.push(b);
        let purse = world.purse(Some(owner));
        let short = (purse as f32) < c.short_cover_days * bill as f32 || unpaid >= c.short_unpaid_days;
        if !short {
            if (purse as f32) >= c.recover_days * bill as f32 {
                if let Some(x) = world.underwater.biz.remove(&b) {
                    if x.hours_cut {
                        // The hours stay cut (once a business); the rest resets.
                        let keep = Biz { hours_cut: true, last_day: x.last_day, ..Default::default() };
                        world.underwater.biz.insert(b, keep);
                    }
                    if x.rounds > 0 && (x.freeze || x.short_days > 0) {
                        let text = format!(
                            "{} is afloat again at {} ({purse}c in hand against {bill}c a day in wages)",
                            world.name_of(owner),
                            world.name_of(b)
                        );
                        world.push_event(EventKind::UnderWater, &[owner, b], text);
                        ownership::jobs2_row(world).uw_afloat += 1;
                    }
                }
            } else if let Some(x) = world.underwater.biz.get_mut(&b) {
                x.short_days = 0;
            }
            continue;
        }
        ownership::jobs2_row(world).uw_short += 1;
        let x = world.underwater.biz.entry(b).or_default();
        x.short_days = x.short_days.saturating_add(1);
        let due = x.short_days >= c.decide_days
            && x.last_day.is_none_or(|d| today.saturating_sub(d) >= u64::from(c.decide_cooldown_days));
        if due {
            decide(world, c, b, owner, bill, &staff);
        }
    }
    // A business that is gone, changed hands or has no staff to pay leaves the book.
    world.underwater.biz.retain(|b, _| seen.contains(b));
}

#[derive(Clone, Copy, Debug)]
enum Choice {
    Hold,
    Hours,
    Layoff,
    Borrow(EntityId, Lender, i64),
    Sell(EntityId, bool),
    Close,
}

fn decide(world: &mut World, c: &UnderwaterCfg, b: EntityId, owner: EntityId, bill: i64, staff: &[EntityId]) {
    let today = world.day();
    let t = traits(world, owner);
    let m = crate::systems::misery::misery(world, owner);
    let Some(x) = world.underwater.biz.get(&b).cloned() else { return };
    let esc = 1.0 + c.escalate * f32::from(x.rounds);
    let mut opts: Vec<(Choice, f32)> = vec![(Choice::Hold, c.w_hold * (0.25 + t.pride))];
    if !x.hours_cut {
        opts.push((Choice::Hours, c.w_hours * (0.5 + t.loyalty)));
    }
    if !staff.is_empty() {
        opts.push((Choice::Layoff, c.w_layoff * (1.5 - t.loyalty)));
    }
    let in_debt = world.underwater.loans.iter().any(|l| l.borrower == owner);
    if !in_debt {
        let (want, min) = (bill * i64::from(c.loan_days), bill * i64::from(c.loan_min_days));
        if let Some((lender, kind, p)) = lender_for(world, c, b, owner, want, min, t) {
            opts.push((Choice::Borrow(lender, kind, p), c.w_borrow * (0.5 + t.courage) * (1.0 - 0.5 * m)));
        }
    }
    if let Some((buyer, front)) = buyer_for(world, c, b, owner, t, m) {
        opts.push((Choice::Sell(buyer, front), c.w_sell * (0.5 + t.greed) * esc));
    }
    opts.push((Choice::Close, c.w_close * (0.25 + m) * esc));
    let total: f32 = opts.iter().map(|o| o.1.max(0.0)).sum();
    let mut u = hash(world, PURPOSE_CHOICE, b, today) * total;
    let mut pick = Choice::Hold;
    for (ch, w) in &opts {
        let w = w.max(0.0);
        if u < w {
            pick = *ch;
            break;
        }
        u -= w;
    }
    if let Some(x) = world.underwater.biz.get_mut(&b) {
        x.last_day = Some(today);
        x.rounds = x.rounds.saturating_add(1);
    }
    let purse = world.purse(Some(owner));
    let till = format!("{purse}c in hand against {bill}c a day in wages");
    match pick {
        Choice::Hold => {
            let text = format!("{} is under water at {} ({till}) and hangs on", world.name_of(owner), world.name_of(b));
            world.push_event(EventKind::UnderWater, &[owner, b], text);
            ownership::jobs2_row(world).uw_hold += 1;
        }
        Choice::Hours => cut_hours(world, c, b, owner, staff, &till),
        Choice::Layoff => layoff(world, c, b, owner, staff, &till),
        Choice::Borrow(lender, kind, p) => borrow(world, c, b, owner, p, lender, kind),
        Choice::Sell(buyer, front) => sell(world, c, b, owner, buyer, front, &till),
        Choice::Close => close(world, c, b, owner, &till),
    }
}

// ---------------------------------------------------------------------------
// Cut back
// ---------------------------------------------------------------------------

/// Trim `cut` of a shift's total length off its end (the last window first);
/// at least an hour stays.
fn trim_shifts(shifts: &mut Vec<(u16, u16)>, cut: f32) {
    let total: u32 = shifts.iter().map(|&(s, e)| u32::from(e.saturating_sub(s))).sum();
    let mut left = ((total as f32 * cut).round() as u32).min(total.saturating_sub(60));
    while left > 0 {
        let only = shifts.len() == 1;
        let Some(last) = shifts.last_mut() else { break };
        let len = u32::from(last.1.saturating_sub(last.0));
        if len > left || only {
            last.1 -= u16::try_from(left.min(len.saturating_sub(1))).unwrap_or(0);
            break;
        }
        left -= len;
        shifts.pop();
    }
}

fn cut_hours(world: &mut World, c: &UnderwaterCfg, b: EntityId, owner: EntityId, staff: &[EntityId], till: &str) {
    let (mut before, mut after) = (0i64, 0i64);
    for &s in staff {
        if let Some(j) = world.comp_mut::<Job>(s) {
            before += j.wage_per_day;
            trim_shifts(&mut j.shifts, c.hours_cut);
            j.wage_per_day = ((j.wage_per_day as f32 * (1.0 - c.hours_cut)).round() as i64).max(1);
            after += j.wage_per_day;
        }
    }
    if let Some(x) = world.underwater.biz.get_mut(&b) {
        x.hours_cut = true;
        x.freeze = true;
    }
    let text = format!(
        "{} cut the hours at {} ({till}): {} on short shifts, the wages {before}c -> {after}c a day; no hiring",
        world.name_of(owner),
        world.name_of(b),
        staff.len()
    );
    let mut actors = vec![owner, b];
    actors.extend(staff.iter().copied().take(4));
    world.push_event(EventKind::UnderWater, &actors, text);
    ownership::jobs2_row(world).uw_hours += 1;
}

/// Dismiss `s` from `b` with `why`; wages owed become the owner's debt.
fn let_go(world: &mut World, b: EntityId, owner: EntityId, s: EntityId, why: &str) {
    let Some(job) = world.comp::<Job>(s).cloned() else { return };
    let owed = job.wage_per_day * i64::from(job.days_unpaid);
    let owed_txt = if owed > 0 && !household(world, owner, s) { format!(", owed {owed}c") } else { String::new() };
    let text = if s == owner {
        format!("{} is out of work at his own {}: {why}", world.name_of(owner), world.name_of(b))
    } else {
        format!(
            "{} let {} go at {} as {}: {why}{owed_txt}",
            world.name_of(owner),
            world.name_of(s),
            world.name_of(b),
            job.role.label()
        )
    };
    let foreman = (s != owner).then_some(owner);
    crate::systems::economy::dismiss_by(world, s, Some(b), text, EventKind::LaidOff, foreman);
    ownership::jobs2_row(world).uw_layoffs += 1;
    if owed > 0 && !household(world, owner, s) {
        owe(world, owner, s, owed);
        let w = crate::systems::misery::cfg(world).map_or(0.0, |m| m.stiff_grudge);
        if w > 0.0 {
            crate::systems::grudges::add(world, s, owner, crate::word::GrudgeCause::Betrayed, w, 0);
        }
        note_stiffed(world, s, owner, Some(b));
    }
}

fn layoff(world: &mut World, c: &UnderwaterCfg, b: EntityId, owner: EntityId, staff: &[EntityId], till: &str) {
    let mut by_hire: Vec<(u64, EntityId)> =
        staff.iter().filter_map(|&s| world.comp::<Job>(s).map(|j| (j.hired_tick, s))).collect();
    // Newest first (ties the higher id).
    by_hire.sort_unstable_by(|a, b| b.cmp(a));
    let n = ((staff.len() as f32 * c.layoff_share).ceil() as usize).clamp(1, staff.len());
    let why = format!("the till could not meet the wages ({till})");
    for &(_, s) in by_hire.iter().take(n) {
        let_go(world, b, owner, s, &why);
    }
    if let Some(x) = world.underwater.biz.get_mut(&b) {
        x.freeze = true;
    }
}

// ---------------------------------------------------------------------------
// Borrow
// ---------------------------------------------------------------------------

/// The local gang for `b` that can spare `need`: the district's controller,
/// then the gangs holding Blocks there (most first), then the nearest
/// Hideouts (at 20k a gang's turf is a Block or two and the holders are
/// often broke, so the gang next door answers too).
fn gang_for(world: &World, c: &UnderwaterCfg, b: EntityId, need: i64) -> Option<EntityId> {
    let d = world.district_of_building(b);
    let door = world.comp::<Building>(b)?.door;
    let ctl = match world.districts.get(d.index()).map(|x| x.control) {
        Some(Controller::Gang(g)) => Some(g),
        _ => None,
    };
    let mut cands: Vec<(u8, u32, EntityId)> = world
        .gang_list()
        .iter()
        .copied()
        .filter(|&g| world.has::<Gang>(g))
        .map(|g| {
            let held = crate::systems::gang::held_districts(world, g).into_iter().find(|&(x, _)| x == d).map(|x| x.1);
            let dist = world
                .hideout_of(g)
                .and_then(|h| world.comp::<Building>(h))
                .map_or(u32::MAX, |h| h.door.manhattan(door));
            let rank = if ctl == Some(g) {
                0
            } else if held.is_some() {
                1
            } else {
                2
            };
            (rank, if rank == 1 { u32::MAX - held.unwrap_or(0) as u32 } else { dist }, g)
        })
        .collect();
    cands.sort_unstable();
    cands.into_iter().map(|x| x.2).find(|&g| gang_spare(world, c, g) >= need.max(1))
}

fn gang_spare(world: &World, c: &UnderwaterCfg, g: EntityId) -> i64 {
    world.comp::<Gang>(g).map_or(0, |x| x.treasury - c.gang_reserve)
}

/// A lender for `want` coins: the gang that answers for the district, a
/// loan shark of the district or a corp (the lawful only), each lending what
/// it can spare up to `want`, never under `min`; picked on the owner's
/// lawfulness. `(lender, kind, principal)`.
fn lender_for(
    world: &World,
    c: &UnderwaterCfg,
    b: EntityId,
    owner: EntityId,
    want: i64,
    min: i64,
    t: Traits,
) -> Option<(EntityId, Lender, i64)> {
    if want <= 0 {
        return None;
    }
    let min = min.max(1);
    let mut opts: Vec<(EntityId, Lender, i64, f32)> = Vec::new();
    if let Some(g) = gang_for(world, c, b, min) {
        let p = want.min(gang_spare(world, c, g));
        if p >= min {
            opts.push((g, Lender::Gang, p, 1.0 - t.lawful));
        }
    }
    // A shark: the richest lawless free adult living in the district.
    let d = world.district_of_building(b);
    let shark = world
        .districts
        .get(d.index())
        .map(|x| x.residents.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&a| !household(world, owner, a) && person(world, a))
        .filter(|&a| world.comp::<Personality>(a).is_some_and(|p| p.lawfulness < c.shark_lawful_max))
        .filter(|&a| crate::systems::contracts::free_adult(world, a) && !crate::systems::founding::is_exec(world, a))
        .filter_map(|a| world.comp::<Wallet>(a).map(|w| (w.coins, std::cmp::Reverse(a))))
        .max()
        .map(|(coins, std::cmp::Reverse(a))| (a, (coins as f32 / c.shark_wealth_mult.max(1.0)) as i64));
    if let Some((s, spare)) = shark {
        let p = want.min(spare);
        if p >= min {
            opts.push((s, Lender::Shark, p, 0.8 * (1.0 - t.lawful)));
        }
    }
    if t.lawful >= c.corp_lawful_min {
        let need = (want as f32 * c.corp_spare_mult) as i64;
        let corp = world
            .corps()
            .into_iter()
            .filter_map(|k| world.comp::<Corp>(k).map(|x| (x.treasury - x.treasury_ref, std::cmp::Reverse(k))))
            .filter(|&(spare, _)| spare >= need)
            .max()
            .map(|(_, std::cmp::Reverse(k))| k);
        if let Some(k) = corp {
            opts.push((k, Lender::Corp, want, t.lawful));
        }
    }
    let total: f32 = opts.iter().map(|o| o.3.max(0.0)).sum();
    if opts.is_empty() || total <= 0.0 {
        return None;
    }
    let mut u = hash(world, PURPOSE_LENDER, b, world.day()) * total;
    for &(id, k, p, w) in &opts {
        if u < w.max(0.0) {
            return Some((id, k, p));
        }
        u -= w.max(0.0);
    }
    opts.last().map(|&(id, k, p, _)| (id, k, p))
}

fn borrow(
    world: &mut World,
    c: &UnderwaterCfg,
    b: EntityId,
    owner: EntityId,
    principal: i64,
    lender: EntityId,
    kind: Lender,
) {
    let paid = ownership::pay(world, Some(lender), Some(owner), principal, Flow::Finance);
    if paid <= 0 {
        return;
    }
    let rate = match kind {
        Lender::Gang => c.rate_gang,
        Lender::Shark => c.rate_shark,
        Lender::Corp => c.rate_corp,
    };
    let owe_total = (paid as f32 * (1.0 + rate)).round() as i64;
    let weeks = i64::from(c.loan_weeks.max(1));
    let installment = (owe_total + weeks - 1) / weeks;
    let today = world.day();
    world.underwater.loans.push(Loan {
        building: b,
        borrower: owner,
        lender,
        kind,
        principal: paid,
        left: owe_total,
        installment,
        next_due: today + 7,
        missed: 0,
    });
    let text = format!(
        "{} borrowed {paid}c from {} ({}) to keep {} open: {owe_total}c back at {installment}c a week",
        world.name_of(owner),
        name(world, lender),
        kind.label(),
        world.name_of(b)
    );
    let mut actors = vec![owner, lender, b];
    if kind == Lender::Gang {
        actors.extend(world.comp::<Gang>(lender).and_then(|g| g.leader));
    }
    world.push_event(EventKind::Loan, &actors, text);
    ownership::jobs2_row(world).uw_loans += 1;
}

fn loans_daily(world: &mut World, c: &UnderwaterCfg) {
    let today = world.day();
    let mut i = 0;
    while i < world.underwater.loans.len() {
        let l = world.underwater.loans[i].clone();
        let gone = match l.kind {
            Lender::Gang => !world.has::<Gang>(l.lender),
            Lender::Shark => !living(world, l.lender),
            Lender::Corp => !world.has::<Corp>(l.lender),
        };
        if !living(world, l.borrower) || gone {
            world.underwater.loans.remove(i);
            continue;
        }
        if today < l.next_due {
            i += 1;
            continue;
        }
        let due = (l.installment * i64::from(l.missed + 1)).min(l.left);
        let pay = due.min(world.purse(Some(l.borrower)).max(0));
        let paid =
            if pay > 0 { ownership::pay(world, Some(l.borrower), Some(l.lender), pay, Flow::Finance) } else { 0 };
        let mut x = l.clone();
        x.left -= paid;
        x.next_due = today + 7;
        if paid >= due {
            x.missed = 0;
        } else {
            x.missed = x.missed.saturating_add(1);
            let text = format!(
                "{} missed a payment on the loan from {} for {} ({paid}c of {due}c; {}c still owed)",
                world.name_of(l.borrower),
                name(world, l.lender),
                world.name_of(l.building),
                x.left
            );
            world.push_event(EventKind::Loan, &[l.borrower, l.lender, l.building], text);
            ownership::jobs2_row(world).uw_missed += 1;
        }
        if x.left <= 0 {
            let text = format!(
                "{} paid off the loan from {} ({}c borrowed)",
                world.name_of(l.borrower),
                name(world, l.lender),
                l.principal
            );
            world.push_event(EventKind::Loan, &[l.borrower, l.lender, l.building], text);
            ownership::jobs2_row(world).uw_repaid += 1;
            world.underwater.loans.remove(i);
            continue;
        }
        let seize_at = if l.kind == Lender::Corp { c.corp_repo_misses } else { c.seize_misses };
        if x.missed >= seize_at {
            world.underwater.loans.remove(i);
            seize(world, &x);
            continue;
        }
        world.underwater.loans[i] = x.clone();
        if x.missed > 0 && l.kind != Lender::Corp {
            send_collector(world, c, &x);
        }
        i += 1;
    }
}

/// The collector for a loan: the gang's toughest free member (its leader
/// last), or the shark himself.
fn collector(world: &World, l: &Loan) -> Option<EntityId> {
    match l.kind {
        Lender::Shark => crate::systems::contracts::free_adult(world, l.lender).then_some(l.lender),
        Lender::Gang => {
            let g = world.comp::<Gang>(l.lender)?;
            let mut best: Option<(bool, f32, EntityId)> = None;
            for &m in &g.members {
                if m == l.borrower || !crate::systems::contracts::free_adult(world, m) {
                    continue;
                }
                let key = (g.leader == Some(m), crate::systems::law::fighting(world, m), m);
                let better = best.is_none_or(|b| (key.0, -key.1) < (b.0, -b.1));
                if better {
                    best = Some(key);
                }
            }
            best.map(|x| x.2)
        }
        Lender::Corp => None,
    }
}

/// A visit due today between the confrontation hours; `false` with the
/// visits off (the caller resolves at once) or the visitor busy.
fn schedule(world: &mut World, c: &UnderwaterCfg, who: EntityId, kin: EntityId, why: VisitWhy) -> bool {
    if !crate::systems::household::on(world) || world.visits.contains_key(&who) {
        return false;
    }
    let span = c.confront_hour_to.saturating_sub(c.confront_hour_from).max(1);
    let u = hash(world, PURPOSE_HOUR, who, world.day() ^ u64::from(kin.index));
    let hour = c.confront_hour_from + ((u * f32::from(span)) as u8).min(span - 1);
    world.visits.insert(who, PendingVisit { kin, why, hour });
    true
}

fn send_collector(world: &mut World, c: &UnderwaterCfg, l: &Loan) {
    // Nobody is dunned in the cells.
    if !crate::systems::contracts::free_adult(world, l.borrower) {
        return;
    }
    let Some(who) = collector(world, l) else { return };
    if world.visits.contains_key(&who) {
        return;
    }
    if !schedule(world, c, who, l.borrower, VisitWhy::Dun) && !crate::systems::household::on(world) {
        dunned(world, who, l.borrower, false);
    }
}

/// The lender takes the business for the debt (a gang's becomes a front).
fn seize(world: &mut World, l: &Loan) {
    let owns = world.owner_of(l.building) == Some(l.borrower);
    if !owns {
        let text = format!(
            "{} wrote off {}'s debt ({}c): nothing left to take",
            name(world, l.lender),
            world.name_of(l.borrower),
            l.left
        );
        world.push_event(EventKind::Loan, &[l.borrower, l.lender], text);
        return;
    }
    world.underwater.shut.remove(&l.building);
    world.underwater.biz.remove(&l.building);
    crate::systems::corps::move_building(world, l.building, Some(l.lender));
    let front = l.kind == Lender::Gang;
    if front {
        make_front(world, l.building, l.lender);
    }
    let text = format!(
        "{} took {} from {} for the {}c still owed{}",
        name(world, l.lender),
        world.name_of(l.building),
        world.name_of(l.borrower),
        l.left,
        if front { " (a front now)" } else { "" }
    );
    world.push_event(EventKind::Repossessed, &[l.borrower, l.lender, l.building], text);
    ownership::jobs2_row(world).uw_seized += 1;
}

/// A collector's call (a `Visit` arrived, or a Statistical one at its hour).
pub fn dunned(world: &mut World, who: EntityId, owner: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let gang = world.gang_of(who);
    let Some(i) = world
        .underwater
        .loans
        .iter()
        .position(|l| l.borrower == owner && l.missed > 0 && (l.lender == who || Some(l.lender) == gang))
    else {
        return;
    };
    let l = world.underwater.loans[i].clone();
    let arrears = (l.installment * i64::from(l.missed)).min(l.left);
    let purse = world.purse(Some(owner)).max(0);
    let place = world
        .comp::<Position>(owner)
        .and_then(|p| p.building)
        .map_or_else(|| "on the street".to_string(), |b| format!("at {}", world.name_of(b)));
    let how = if walked { "called" } else { "called (off screen)" };
    let whose = if l.lender == who { "the loan".to_string() } else { format!("{}'s loan", name(world, l.lender)) };
    if purse >= arrears {
        let paid = ownership::pay(world, Some(owner), Some(l.lender), arrears, Flow::Finance);
        if let Some(x) = world.underwater.loans.get_mut(i) {
            x.left -= paid;
            x.missed = 0;
        }
        let text = format!(
            "{} {how} {place} to collect on {whose}: {} paid the {paid}c in arrears",
            world.name_of(who),
            world.name_of(owner)
        );
        world.push_event(EventKind::Loan, &[who, owner, l.lender], text);
        return;
    }
    let took = if purse > 0 { ownership::pay(world, Some(owner), Some(l.lender), purse, Flow::Finance) } else { 0 };
    if let Some(x) = world.underwater.loans.get_mut(i) {
        x.left -= took;
    }
    let tile = world.comp::<Position>(owner).map_or_else(Default::default, |p| p.tile);
    let mods = crate::systems::law::FightMods { kill_mult: c.collector_kill_mult, ..Default::default() };
    let (winner, loser, died) = crate::systems::law::resolve_fight_mods(world, who, owner, mods);
    let text = format!(
        "{} {how} {place} to collect on {whose}: {} had {took}c of the {arrears}c; {} gave a beating{}",
        world.name_of(who),
        world.name_of(owner),
        world.name_of(who),
        if winner != who {
            format!(" (fought back: {} won)", world.name_of(winner))
        } else if died && loser == owner {
            ", to death".to_string()
        } else {
            String::new()
        }
    );
    world.push_event(EventKind::Loan, &[who, owner, l.lender], text);
    ownership::jobs2_row(world).uw_beatings += 1;
    crime(world, &c, who, owner, died && loser == owner, tile);
}

/// The Assault (or the Murder) of a scene, the victim's grudge and report.
fn crime(
    world: &mut World,
    c: &UnderwaterCfg,
    a: EntityId,
    b: EntityId,
    murder: bool,
    tile: crate::components::TilePos,
) {
    if !living(world, a) {
        return;
    }
    let kind = if murder { Crime::Murder } else { Crime::Assault };
    if murder {
        let t = format!("{} killed {}", world.name_of(a), world.name_of(b));
        world.push_event(EventKind::Murder, &[a, b], t);
    }
    crate::systems::law::raise_crime_on(world, a, (!murder).then_some(b), Some(b), kind, tile);
    if living(world, b) {
        crate::systems::grudges::add(world, b, a, crate::word::GrudgeCause::Assaulted, 0.5, 0);
        if traits(world, b).lawful >= c.victim_report_lawful {
            crate::systems::law::file_report(world, kind, a, Some(b));
        }
    }
}

// ---------------------------------------------------------------------------
// Sell
// ---------------------------------------------------------------------------

fn sale_price(world: &World, b: EntityId, mult: f32) -> i64 {
    let kind = world.comp::<Building>(b).map_or(BuildingKind::Lot, |bd| bd.kind);
    ((ownership::value(world, kind) as f32 * mult).round() as i64).max(1)
}

fn buyer_for(
    world: &World,
    c: &UnderwaterCfg,
    b: EntityId,
    owner: EntityId,
    t: Traits,
    m: f32,
) -> Option<(EntityId, bool)> {
    let kind = world.comp::<Building>(b)?.kind;
    if ownership::value(world, kind) <= 0 {
        return None;
    }
    let price = sale_price(world, b, c.sell_price_mult);
    let front_price = sale_price(world, b, c.front_price_mult);
    let niche = crate::systems::corp_brain::niche_of_kind(&world.config, kind);
    let corp = niche.and_then(|n| {
        world
            .corps()
            .into_iter()
            .filter_map(|k| {
                let x = world.comp::<Corp>(k)?;
                (x.niches.contains(&n) && x.treasury - x.treasury_ref >= price)
                    .then_some((x.treasury - x.treasury_ref, std::cmp::Reverse(k)))
            })
            .max()
            .map(|(_, std::cmp::Reverse(k))| k)
    });
    let gang = (t.lawful <= c.front_lawful_max || m >= c.front_misery)
        .then(|| gang_for(world, c, b, front_price))
        .flatten()
        .filter(|&g| world.gang_of(owner).is_none_or(|x| x == g));
    match (corp, gang) {
        (Some(k), Some(g)) => {
            let wc = 0.5 + t.lawful;
            let wg = 1.5 - t.lawful;
            Some(if hash(world, PURPOSE_BUYER, b, world.day()) * (wc + wg) < wc { (k, false) } else { (g, true) })
        }
        (Some(k), None) => Some((k, false)),
        (None, Some(g)) => Some((g, true)),
        (None, None) => None,
    }
}

/// `b` is `gang`'s front: a leisure venue's `front_of` too (the L2 dealers
/// and the leader's round read it).
fn make_front(world: &mut World, b: EntityId, gang: EntityId) {
    world.underwater.fronts.insert(b, gang);
    if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
        v.front_of = Some(gang);
    }
}

#[allow(clippy::too_many_arguments)]
fn sell(world: &mut World, c: &UnderwaterCfg, b: EntityId, owner: EntityId, buyer: EntityId, front: bool, till: &str) {
    let price = sale_price(world, b, if front { c.front_price_mult } else { c.sell_price_mult });
    let why = if front { format!("a fire sale as a front: {till}") } else { format!("a fire sale: {till}") };
    if !crate::systems::corps::acquire(world, buyer, b, price, &why) {
        return;
    }
    world.underwater.biz.remove(&b);
    if front {
        make_front(world, b, buyer);
        let leader = world.comp::<Gang>(buyer).and_then(|g| g.leader);
        let text = format!(
            "{} sold {} to {} as a front for {price}c ({till})",
            world.name_of(owner),
            world.name_of(b),
            name(world, buyer)
        );
        let mut actors = vec![owner, buyer, b];
        actors.extend(leader);
        world.push_event(EventKind::UnderWater, &actors, text);
        ownership::jobs2_row(world).uw_sold_gang += 1;
    } else {
        ownership::jobs2_row(world).uw_sold_corp += 1;
    }
}

// ---------------------------------------------------------------------------
// Close
// ---------------------------------------------------------------------------

fn close(world: &mut World, c: &UnderwaterCfg, b: EntityId, owner: EntityId, till: &str) {
    let staff: Vec<EntityId> = world.staff_of(b).to_vec();
    let why = format!("{} is shut ({till})", world.name_of(b));
    for s in staff {
        let_go(world, b, owner, s, &why);
    }
    world.vacancies.remove(&b);
    let until = world.tick + 3650 * TICKS_PER_DAY;
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.closed_until = Some(until);
    }
    let price = sale_price(world, b, c.shut_price_mult);
    let today = world.day();
    world.underwater.biz.remove(&b);
    world.underwater.shut.insert(b, Shut { since: today, price });
    let text = format!(
        "{} shut {} for good ({till}); it stands empty, for sale at {price}c",
        world.name_of(owner),
        world.name_of(b)
    );
    world.push_event(EventKind::UnderWater, &[owner, b], text);
    ownership::jobs2_row(world).uw_closed += 1;
}

/// A would-be founder for a shut business: a free adult, no gang, no exec,
/// not its owner's household, holding the price and `buyer_reserve_days` of
/// its old wage bill on top, jobless or paid no more than the dole; the
/// richest first.
fn shut_daily(world: &mut World, c: &UnderwaterCfg) {
    if world.underwater.shut.is_empty() {
        return;
    }
    let today = world.day();
    let shut: Vec<(EntityId, Shut)> = world.underwater.shut.iter().map(|(&b, s)| (b, s.clone())).collect();
    // Shut buildings that went derelict, changed hands or are gone leave the list.
    let mut live = Vec::new();
    for (b, s) in shut {
        let ok = world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict)
            && world.owner_of(b).is_some_and(|o| person(world, o));
        if !ok {
            world.underwater.shut.remove(&b);
            reopen(world, b);
            continue;
        }
        live.push((b, s));
    }
    if live.is_empty() {
        return;
    }
    let min_price = live.iter().map(|x| x.1.price).min().unwrap_or(0);
    let dole = i64::from(world.levers.dole_per_day);
    // scan-ok: daily, only with a shut business standing
    let mut buyers: Vec<(i64, EntityId)> = world
        .citizens()
        .into_iter()
        .filter(|&a| person(world, a) && crate::systems::demography::is_adult(world, a))
        .filter(|&a| !world.has::<GangMember>(a) && !world.has::<Sentence>(a))
        .filter(|&a| world.comp::<Job>(a).is_none_or(|j| j.wage_per_day <= dole))
        .filter_map(|a| world.comp::<Wallet>(a).map(|w| (w.coins, a)))
        .filter(|&(coins, _)| coins >= min_price)
        .collect();
    buyers.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut sold = 0u32;
    for (b, s) in live {
        let owner = world.owner_of(b);
        if sold < c.shut_buys_per_day {
            let reserve = owner.map_or(0, |o| payroll_hint(world, b, o)) * i64::from(c.buyer_reserve_days);
            let pick = buyers
                .iter()
                .position(|&(coins, a)| {
                    coins >= s.price + reserve
                        && owner.is_none_or(|o| !household(world, o, a))
                        && !crate::systems::founding::is_exec(world, a)
                        && hash(world, PURPOSE_BUYER, a, today) < 0.5
                })
                .map(|k| buyers.remove(k).1);
            if let Some(a) = pick {
                if crate::systems::corps::acquire(world, a, b, s.price, "took the shut business over") {
                    world.underwater.shut.remove(&b);
                    reopen(world, b);
                    if let Some(br) = world.comp_mut::<Brain>(a) {
                        br.last_found_day = Some(today);
                    }
                    let seller = owner.map_or_else(|| "the city".to_string(), |o| world.name_of(o));
                    let text = format!(
                        "{} reopened {} (bought it shut from {seller} for {}c)",
                        world.name_of(a),
                        world.name_of(b),
                        s.price
                    );
                    let mut actors = vec![a, b];
                    actors.extend(owner);
                    world.push_event(EventKind::Founded, &actors, text);
                    ownership::jobs2_row(world).uw_reopened += 1;
                    sold += 1;
                    continue;
                }
            }
        }
        let kind = world.comp::<Building>(b).map(|bd| bd.kind);
        if today.saturating_sub(s.since) >= u64::from(c.shut_derelict_days)
            && kind.is_some_and(crate::systems::street::can_go_derelict)
        {
            world.underwater.shut.remove(&b);
            reopen(world, b);
            crate::systems::street::make_derelict(world, b, "shut and unsold");
        }
    }
}

/// A shut building's old wage bill, as the staff places it posts (the
/// buyer's reserve): the kind's staff × the role's wage.
fn payroll_hint(world: &World, b: EntityId, _owner: EntityId) -> i64 {
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return 0 };
    let staff = i64::from(world.config.buildings.for_kind(kind).staff);
    staff * ownership::role_for(kind).map_or(0, |r| world.config.economy.wage(r))
}

fn reopen(world: &mut World, b: EntityId) {
    let now = world.tick;
    if let Some(bd) = world.comp_mut::<Building>(b) {
        if bd.closed_until.is_some_and(|t| t > now + 30 * TICKS_PER_DAY) {
            bd.closed_until = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Fronts
// ---------------------------------------------------------------------------

fn fronts_daily(world: &mut World, c: &UnderwaterCfg) {
    let today = world.day();
    let fronts: Vec<(EntityId, EntityId)> = world.underwater.fronts.iter().map(|(&b, &g)| (b, g)).collect();
    for (b, g) in fronts {
        let standing = world.comp::<Building>(b).is_some_and(|bd| !bd.demolished && !bd.derelict);
        if !standing || world.owner_of(b) != Some(g) || !world.has::<Gang>(g) {
            world.underwater.fronts.remove(&b);
            continue;
        }
        // The staff are drawn in.
        let staff: Vec<EntityId> = world.staff_of(b).to_vec();
        for s in staff {
            if world.has::<GangMember>(s)
                || world.has::<Sentence>(s)
                || !crate::systems::demography::is_adult(world, s)
                || crate::systems::founding::is_exec(world, s)
            {
                continue;
            }
            let p = c.front_join_p * (1.0 - traits(world, s).lawful);
            if hash(world, PURPOSE_JOIN, s, today) < p {
                crate::systems::gang::enlist(world, s, g);
                let text = format!(
                    "{} was drawn into {} working its front {}",
                    world.name_of(s),
                    name(world, g),
                    world.name_of(b)
                );
                world.push_event(EventKind::UnderWater, &[s, g, b], text);
                ownership::jobs2_row(world).uw_front_joins += 1;
            }
        }
        if today % 7 != u64::from(b.index) % 7 {
            continue;
        }
        // The week's take washed through the till.
        let week = world.comp::<Gang>(g).map_or(0, |x| x.tribute_week.max(0));
        let washed = (week as f32 * c.launder_share).floor() as i64;
        if washed > 0 {
            if let Some(bd) = world.comp_mut::<Building>(b) {
                bd.revenue_today += washed;
            }
            let text = format!("{} washed {washed}c of the week's take through {}", name(world, g), world.name_of(b));
            let mut actors = vec![g, b];
            actors.extend(world.comp::<Gang>(g).and_then(|x| x.leader));
            world.push_event(EventKind::UnderWater, &actors, text);
            ownership::jobs2_row(world).uw_laundered += washed;
        }
        // The gang's rival hits it (deviation: not only a rival with turf in
        // the district; at 20k a gang holds a Block or two).
        let Some(r) = world.rival_of(g).filter(|&r| world.has::<Gang>(r)) else { continue };
        if hash(world, PURPOSE_HIT, b, today) >= c.rival_hit_p {
            continue;
        }
        let take = c.rival_take.min(world.purse(Some(g)).max(0));
        let took = if take > 0 { ownership::pay(world, Some(g), Some(r), take, Flow::Robbery) } else { 0 };
        let until = world.tick + u64::from(c.rival_close_days) * TICKS_PER_DAY;
        if let Some(bd) = world.comp_mut::<Building>(b) {
            bd.closed_until = Some(bd.closed_until.map_or(until, |t| t.max(until)));
        }
        let text = format!(
            "{} hit {}'s front {}: took {took}c and shut it {} days",
            name(world, r),
            name(world, g),
            world.name_of(b),
            c.rival_close_days
        );
        let mut actors = vec![r, g, b];
        actors.extend(world.comp::<Gang>(r).and_then(|x| x.leader));
        actors.extend(world.comp::<Gang>(g).and_then(|x| x.leader));
        world.push_event(EventKind::Raid, &actors, text);
        ownership::jobs2_row(world).uw_front_hits += 1;
    }
}

/// A gang's bought or seized fronts (any kind), ascending.
pub fn bought_fronts(world: &World, gang: EntityId) -> Vec<EntityId> {
    world.underwater.fronts.iter().filter(|(_, &g)| g == gang).map(|(&b, _)| b).collect()
}

// ---------------------------------------------------------------------------
// The stiffed worker
// ---------------------------------------------------------------------------

fn stiffs_daily(world: &mut World, c: &UnderwaterCfg) {
    let today = world.day();
    let mut i = 0;
    while i < world.underwater.stiffs.len() {
        let s = world.underwater.stiffs[i].clone();
        let due = owed(world, s.owner, s.worker);
        if due <= 0 || !living(world, s.worker) || !living(world, s.owner) {
            world.underwater.stiffs.remove(i);
            continue;
        }
        if s.reported {
            let take = due.min(world.purse(Some(s.owner)).max(0));
            if take > 0 {
                let paid = ownership::pay(world, Some(s.owner), Some(s.worker), take, Flow::Wage);
                settle(world, s.owner, s.worker, paid);
                let left = due - paid;
                let text = format!(
                    "the Hall took {paid}c from {} for {}'s wages{}",
                    world.name_of(s.owner),
                    world.name_of(s.worker),
                    if left > 0 { format!(" ({left}c still owed)") } else { " (paid in full)".to_string() }
                );
                world.push_event(EventKind::Confronted, &[s.worker, s.owner], text);
                ownership::jobs2_row(world).uw_garnished += 1;
                if left <= 0 {
                    world.underwater.stiffs.remove(i);
                    continue;
                }
            }
            i += 1;
            continue;
        }
        let ready = s.tries < c.confront_max
            && crate::systems::contracts::free_adult(world, s.owner)
            && today >= s.since + u64::from(c.confront_wait_days)
            && crate::systems::contracts::free_adult(world, s.worker)
            && !world.visits.contains_key(&s.worker);
        if ready {
            let t = traits(world, s.worker);
            let m = crate::systems::misery::misery(world, s.worker);
            let p = c.confront_p * (0.5 + t.pride + m);
            if hash(world, PURPOSE_CONFRONT, s.worker, today ^ u64::from(s.owner.index)) < p {
                if let Some(x) = world.underwater.stiffs.get_mut(i) {
                    x.tries += 1;
                }
                if !schedule(world, c, s.worker, s.owner, VisitWhy::Confront) && !crate::systems::household::on(world) {
                    confronted(world, s.worker, s.owner, false);
                }
            }
        }
        i += 1;
    }
}

/// The confrontation (a `Visit` arrived, or a Statistical one at its hour):
/// the worker demands the wages the owner owes him.
pub fn confronted(world: &mut World, worker: EntityId, owner: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(i) = world.underwater.stiffs.iter().position(|s| s.worker == worker && s.owner == owner) else { return };
    let due = owed(world, owner, worker);
    if due <= 0 {
        world.underwater.stiffs.remove(i);
        return;
    }
    let here = world.comp::<Position>(owner).and_then(|p| p.building);
    let place = here.map_or_else(|| "on the street".to_string(), |b| format!("at {}", world.name_of(b)));
    let (wn, on) = (world.name_of(worker), world.name_of(owner));
    let how = if walked { format!("walked up to {on}") } else { format!("caught {on} (off screen)") };
    let head = format!("{wn} {how} {place} and demanded the {due}c in wages owed");
    let mut actors = vec![worker, owner];
    actors.extend(here);
    ownership::jobs2_row(world).uw_confronts += 1;
    let t = traits(world, owner);
    let p_pay =
        (c.pay_base + c.pay_lawful * t.lawful + c.pay_loyal * t.loyalty - c.pay_greed * t.greed).clamp(0.0, 1.0);
    let purse = world.purse(Some(owner)).max(0);
    if purse > 0 && hash(world, PURPOSE_PAY, worker, world.day() ^ u64::from(owner.index)) < p_pay {
        let paid = ownership::pay(world, Some(owner), Some(worker), due.min(purse), Flow::Wage);
        settle(world, owner, worker, paid);
        let all = paid >= due;
        let text = format!("{head}: {on} paid {paid}c{}", if all { ", all of it" } else { ", all there was" });
        world.push_event(EventKind::Confronted, &actors, text);
        ownership::jobs2_row(world).uw_paid_up += 1;
        if all {
            world.underwater.stiffs.remove(i);
        }
        return;
    }
    let reason = if purse <= 0 { "had nothing to give" } else { "would not pay" };
    crate::systems::grudges::add(world, worker, owner, crate::word::GrudgeCause::Betrayed, c.confront_grudge, 0);
    let g = crate::systems::grudges::grudge_on(world, worker, owner).map_or(0.0, |x| x.weight);
    if g >= c.fight_grudge {
        let tile = world.comp::<Position>(owner).map_or_else(Default::default, |p| p.tile);
        let mods = crate::systems::law::FightMods { kill_mult: c.confront_kill_mult, ..Default::default() };
        let (winner, loser, died) = crate::systems::law::resolve_fight_mods(world, worker, owner, mods);
        let text = format!(
            "{head}: {on} {reason}; {wn} swung (grudge {g:.2}); {} won{}",
            world.name_of(winner),
            if died { format!(", {} died", world.name_of(loser)) } else { String::new() }
        );
        world.push_event(EventKind::Confronted, &actors, text);
        ownership::jobs2_row(world).uw_confront_fights += 1;
        crime(world, &c, worker, owner, died && loser == owner, tile);
        return;
    }
    if traits(world, worker).lawful >= c.report_lawful {
        if let Some(x) = world.underwater.stiffs.get_mut(i) {
            x.reported = true;
        }
        let text = format!("{head}: {on} {reason}; {wn} reported {on} to the Hall for the wages");
        world.push_event(EventKind::Confronted, &actors, text);
        ownership::jobs2_row(world).uw_reports += 1;
        return;
    }
    let text = format!("{head}: {on} {reason}; {wn} left with nothing (grudge {g:.2})");
    world.push_event(EventKind::Confronted, &actors, text);
}

/// The Visit's urgency for the two calls (0 without the section).
pub fn urgency(world: &World, why: VisitWhy) -> f32 {
    cfg(world).map_or(0.0, |c| match why {
        VisitWhy::Dun => c.dun_urgency,
        _ => c.confront_urgency,
    })
}
