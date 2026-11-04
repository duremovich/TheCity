use super::*;

// ---------------------------------------------------------------------------
// Fraud: the fake job
// ---------------------------------------------------------------------------

/// The fake job (spec § 4 Fraud), daily: at each open Fixer, a regular
/// with deception ≥ 0.6 and lawfulness < 0.3, on a day its keyed draw
/// falls under `fake_p`, sells a job that does not exist to the regular
/// with the fattest wallet (at least `fake_fee`) it has not tried: a
/// Deceive move (`Stake::Coins(fake_fee)`); success takes the fee
/// (`Flow::Coerced`), and the victim learns it after `fake_days`.
pub(super) fn fake_pass(world: &mut World) {
    let day = world.day();
    let fee = world.config.leverage.fake_fee;
    let p = world.config.leverage.fake_p;
    for f in contracts::open_fixers(world) {
        let regulars: Vec<EntityId> =
            world.comp::<crate::contract::Broker>(f).map(|k| k.regulars.keys().copied().collect()).unwrap_or_default();
        for &r in &regulars {
            let ok = world.comp::<Skills>(r).is_some_and(|s| s.deception >= 0.6)
                && world.comp::<Personality>(r).is_some_and(|x| x.lawfulness < 0.3)
                && free(world, r);
            if !ok || draw(world, Purpose::Fake, day, u64::from(r.index)) >= p {
                continue;
            }
            let victim = regulars
                .iter()
                .copied()
                .filter(|&v| v != r && free(world, v) && !tried_recently(world, r, v, tried::FAKE))
                .filter(|&v| !world.fake_jobs.iter().any(|j| j.victim == v) && !wary(world, v))
                .map(|v| (world.purse(Some(v)), v))
                .filter(|&(c, _)| c >= fee)
                .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
                .map(|(_, v)| v);
            if let Some(v) = victim {
                try_fake_job(world, r, v);
            }
        }
    }
}

/// Round 4 T6: a mark who learned of a con (an unsettled `Betrayed` grudge)
/// inside `fake_wary_days` buys no job up front: the same night's second
/// fake job read as one con with two sellers (`vendor_11367 d24 00:09`).
fn wary(world: &World, v: EntityId) -> bool {
    let days = world.config.leverage.fake_wary_days;
    if days == 0 {
        return false;
    }
    let window = Tick::from(days) * TICKS_PER_DAY;
    world.comp::<crate::word::Grudges>(v).is_some_and(|gs| {
        gs.list.iter().any(|g| {
            g.settled.is_none()
                && g.cause == crate::word::GrudgeCause::Betrayed
                && world.tick.saturating_sub(g.since) < window
        })
    })
}

/// One fake job: `faker` sells `victim` a job that does not exist. True
/// when the fee changed hands.
pub fn try_fake_job(world: &mut World, faker: EntityId, victim: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    mark(world, faker, victim, tried::FAKE);
    let fee = world.config.leverage.fake_fee;
    let m = SocialMove { actor: faker, target: victim, kind: MoveKind::Deceive, stake: Stake::Coins(fee) };
    if !moves::resolve(world, &m).success {
        return false;
    }
    let paid = ownership::pay(world, Some(victim), Some(faker), fee, Flow::Coerced);
    if paid <= 0 {
        return false;
    }
    let due = world.tick + Tick::from(world.config.leverage.fake_days.max(1)) * TICKS_PER_DAY;
    world.fake_jobs.push(FakeJob { faker, victim, fee: paid, due });
    world.stats.current.leverage.fake_jobs += 1;
    let text = format!("{} sold {} a job for {paid} up front", world.name_of(faker), world.name_of(victim));
    world.push_event(EventKind::Coerced, &[faker, victim], text);
    true
}

/// Daily: a fake job's victim learns it at its due: a `Betrayed` deed
/// (actor the faker) and the victim's `Betrayed` grudge (`renege_grudge`),
/// and a lawful victim (≥ 0.5) files Fraud.
pub(super) fn fake_due(world: &mut World) {
    let now = world.tick;
    let (due, keep): (Vec<FakeJob>, Vec<FakeJob>) =
        std::mem::take(&mut world.fake_jobs).into_iter().partition(|j| j.due <= now);
    world.fake_jobs = keep;
    for j in due {
        if !law::living(world, j.victim) {
            continue;
        }
        contracts::betrayed(world, j.faker, j.victim);
        let w = world.config.contracts.renege_grudge;
        crate::systems::grudges::add(world, j.victim, j.faker, crate::word::GrudgeCause::Betrayed, w, 0);
        let text = format!(
            "{} learned the job {} sold for {} never existed",
            world.name_of(j.victim),
            world.name_of(j.faker),
            j.fee
        );
        world.push_event(EventKind::Defrauded, &[j.victim, j.faker], text);
        file_fraud(world, j.victim, j.faker);
    }
}

/// Fraud (spec § 4): a defrauded person with lawfulness ≥ 0.5 files
/// `Crime::Fraud` against the fraudster (a person); the rest keep the grudge.
pub fn file_fraud(world: &mut World, victim: EntityId, culprit: EntityId) {
    if !on(world) || !law::living(world, victim) || !world.has::<Identity>(culprit) {
        return;
    }
    if world.comp::<Personality>(victim).is_none_or(|p| p.lawfulness < 0.5) {
        return;
    }
    law::file_report(world, Crime::Fraud, culprit, Some(victim));
    world.stats.current.leverage.frauds_filed += 1;
}

// ---------------------------------------------------------------------------
// Court: the Persuade kind
// ---------------------------------------------------------------------------

/// The person `id` was rejected by at least `court_rejections` times in
/// the last 30 days (its `Rejected` memories), unmarried and living; the
/// most often, ties the lower id.
pub fn court_target(world: &World, id: EntityId) -> Option<EntityId> {
    let m = world.comp::<Memory>(id)?;
    let month = 30 * TICKS_PER_DAY;
    let mut counts: std::collections::BTreeMap<EntityId, u32> = std::collections::BTreeMap::new();
    for e in m.entries.iter() {
        if e.kind == crate::components::MemoryKind::Rejected && world.tick.saturating_sub(e.tick) < month {
            if let Some(s) = e.subject {
                *counts.entry(s).or_default() += 1;
            }
        }
    }
    let need = world.config.leverage.court_rejections;
    counts
        .into_iter()
        .filter(|&(t, n)| n >= need && t != id && law::living(world, t) && world.has::<Identity>(t))
        .filter(|&(t, _)| world.spouse_of(t).is_none() && crate::systems::demography::is_adult(world, t))
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(t, _)| t)
}

/// Daily, one adult in seven (`index % 7`): a suitor under
/// `court_sociability`, unmarried and free, rejected twice by the same
/// person ([`court_target`]), with the quote in its wallet and no live
/// record as buyer, buys a `Persuade` on the courted (brokered at the
/// nearest open Fixer, else direct); `Origin::Court`.
pub(super) fn court_pass(world: &mut World) {
    let day = world.day();
    let soc = world.config.leverage.court_sociability;
    // scan-ok: daily, one adult in seven.
    for a in world.citizens() {
        if !(u64::from(a.index) + day).is_multiple_of(7) {
            continue;
        }
        if world.comp::<Personality>(a).is_none_or(|p| p.sociability >= soc) || !free(world, a) {
            continue;
        }
        if world.spouse_of(a).is_some() {
            continue;
        }
        let Some(t) = court_target(world, a) else { continue };
        if tried_recently(world, a, t, tried::COURT) {
            continue;
        }
        try_court(world, a, t);
    }
}

/// One Court posting: `suitor` buys a `Persuade` on `courted`. True when posted.
pub fn try_court(world: &mut World, suitor: EntityId, courted: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    if world.by_party.get(&suitor).is_some_and(|l| {
        l.iter().any(|id| world.contracts.get(id).is_some_and(|c| c.is_live() && c.buyer == Some(suitor)))
    }) {
        return false;
    }
    mark(world, suitor, courted, tried::COURT);
    let broker = contracts::nearest_fixer(world, suitor).filter(|&f| contracts::fixer_open(world, f));
    let target = Target::Agent(courted);
    let price = contracts::quote(world, ContractKind::Persuade, &target, broker);
    if world.purse(Some(suitor)) < price {
        return false;
    }
    let deadline_days = world.config.contracts.deadline_days.get(ContractKind::Persuade);
    let p = Posting {
        buyer: Some(suitor),
        agent: Some(suitor),
        kind: ContractKind::Persuade,
        target,
        broker,
        deadline_days,
        origin: Origin::Court,
        price: Some(price),
    };
    contracts::post(world, p).is_ok()
}

/// A `Persuade` record at its ledger due (from `contracts::resolve_ledger`):
/// the taker makes a Charm move on the target; success adds
/// `persuade_affinity` (and 0.1 trust) to the target's edge with the buyer
/// and fulfils the record, failure is a failed attempt.
pub fn resolve_persuade(world: &mut World, id: ContractId, taker: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let Some(t) = c.target_agent() else { return };
    let Some(buyer) = c.buyer.or(c.agent) else { return };
    if !law::living(world, t) || !law::living(world, buyer) {
        contracts::settle(world, id, contracts::Settle::Cancelled, "a party is gone");
        return;
    }
    let m = SocialMove { actor: taker, target: t, kind: MoveKind::Charm, stake: Stake::Info { about: buyer } };
    if moves::resolve(world, &m).success {
        // M16b phase 4: a board campaign's Persuade pins the member's vote.
        if c.origin == Origin::Board {
            crate::systems::governance::on_persuaded(world, buyer, t);
            contracts::settle(world, id, contracts::Settle::Fulfilled, "");
            return;
        }
        let aff = world.config.leverage.persuade_affinity;
        crate::systems::social::adjust(world, t, buyer, aff, 0.1);
        contracts::settle(world, id, contracts::Settle::Fulfilled, "");
    } else {
        contracts::fail_attempt(world, id, "the courted would not hear it");
    }
}
