//! M16b phase 4 (docs/M16_CONTRACTS.md § 6): boards, votes and campaigns.
//!
//! A game abstraction: a board is a list of members on a corp
//! (`Governance::Board`), a vote a tally over it. The corps whose roster
//! row is named in `[governance] boards` (every copy of it: `corp_template`)
//! seat a board at the first midnight: the exec as chair plus the
//! `board_size − 1` employees of the highest standing; a dead or departed
//! member is replaced by the next. Every `vote_days`, and at once on the
//! chair's death, the members vote: each for the candidate (the chair and
//! the members with `greed × standing ≥ ambition_min`) of the highest
//! `pref = 0.4 × affinity + 0.3 × standing + 0.3 × (honour − 0.5) × 2`
//! unless pinned (a campaign's Persuade, or a `Vote` coercion). Ties go to
//! the chair; a new chair becomes the exec. `campaign_days` before a vote
//! the strongest challenger within two votes leans on the swing member of
//! the closest margin, one a day. A board corp's brain reads the members'
//! mean Personality (the chair counted twice).
//!
//! `[governance] enabled = false` (`CITYSIM_PHASE4_OFF=1`): no board is
//! seated, and every corp is a dictatorship as in phase 3.

use crate::components::{Corp, Governance, Personality};
use crate::contract::{
    CoercionStatus, CoercionWhy, ContractId, ContractKind, Demand, Leverage, Origin, Posting, Target,
};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::systems::ownership::{self, Flow};
use crate::systems::{contracts, law, leverage, moves, reputation};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::word::{MoveKind, SocialMove, Stake};
use crate::world::World;

/// Is the switch in flight on?
pub fn on(world: &World) -> bool {
    world.config.governance.enabled
}

/// The hour of the day the campaign and the scheduled vote run.
const NOON: u16 = 12 * 60;

/// Is `corp`'s roster row one with a board?
pub fn wants_board(world: &World, corp: EntityId) -> bool {
    let t = world.corp_template(corp);
    world.config.governance.boards.iter().any(|b| b == t)
}

/// `(members, chair)` of a board corp.
pub fn board_of(world: &World, corp: EntityId) -> Option<(Vec<EntityId>, Option<EntityId>)> {
    match &world.comp::<Corp>(corp)?.governance {
        Governance::Board { members, chair, .. } => Some((members.clone(), *chair)),
        Governance::Dictator => None,
    }
}

/// The corps with a board.
pub fn board_corps(world: &World) -> Vec<EntityId> {
    world.corps().into_iter().filter(|&c| board_of(world, c).is_some()).collect()
}

/// The Personality a corp's brain reads: a board's mean (the chair
/// weighted 2) with governance on, else the exec's.
pub fn personality(world: &World, corp: EntityId) -> Option<Personality> {
    let c = world.comp::<Corp>(corp)?;
    let exec = || c.exec.and_then(|e| world.comp::<Personality>(e)).cloned();
    if !on(world) {
        return exec();
    }
    let Governance::Board { members, chair, .. } = &c.governance else { return exec() };
    let mut sum = [0.0f32; 6];
    let mut w = 0.0f32;
    for &m in members {
        let Some(p) = world.comp::<Personality>(m) else { continue };
        let k = if Some(m) == *chair { 2.0 } else { 1.0 };
        for (s, v) in sum.iter_mut().zip([p.lawfulness, p.greed, p.pride, p.sociability, p.courage, p.loyalty]) {
            *s += k * v;
        }
        w += k;
    }
    if w <= 0.0 {
        return exec();
    }
    Some(Personality {
        lawfulness: sum[0] / w,
        greed: sum[1] / w,
        pride: sum[2] / w,
        sociability: sum[3] / w,
        courage: sum[4] / w,
        loyalty: sum[5] / w,
    })
}

fn employed_at(world: &World, a: EntityId, corp: EntityId) -> bool {
    world
        .comp::<crate::components::Job>(a)
        .and_then(|j| j.employer)
        .and_then(|e| world.owner_of(e))
        .is_some_and(|o| o == corp)
}

fn standing(world: &World, a: EntityId) -> f32 {
    reputation::rep(world, a).standing
}

/// The next members by standing (ties the lower id): living adult
/// employees not on the board, not the chair.
fn next_members(world: &World, corp: EntityId, have: &[EntityId], n: usize) -> Vec<EntityId> {
    let mut v: Vec<(f32, EntityId)> = ownership::employees_of(world, corp)
        .into_iter()
        .filter(|a| !have.contains(a) && law::living(world, *a) && crate::systems::demography::is_adult(world, *a))
        .map(|a| (standing(world, a), a))
        .collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    v.into_iter().take(n).map(|(_, a)| a).collect()
}

/// Seat `corp`'s board: the exec as chair plus the `board_size − 1`
/// employees of the highest standing. False without an exec.
pub fn seat(world: &mut World, corp: EntityId) -> bool {
    let Some(exec) = world.comp::<Corp>(corp).and_then(|c| c.exec).filter(|&e| law::living(world, e)) else {
        return false;
    };
    let size = world.config.governance.board_size.max(1);
    let mut members = vec![exec];
    members.extend(next_members(world, corp, &members, size - 1));
    members.sort();
    let now = world.tick;
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.governance =
            Governance::Board { members, chair: Some(exec), last_vote: now, votes: Vec::new(), pinned: Vec::new() };
    }
    world.stats.current.gov.boards = board_corps(world).len() as u32;
    true
}

/// Hourly: boards seated at midnight (or the first pass), members kept up,
/// the chair's death voted on at once; at noon the campaign and the
/// scheduled vote.
pub fn run(world: &mut World) {
    if !on(world) || !world.tick.is_multiple_of(TICKS_PER_HOUR) {
        return;
    }
    let tod = crate::time::tick_of_day(world.tick);
    if tod == 0 {
        for c in world.corps() {
            if wants_board(world, c) && board_of(world, c).is_none() {
                seat(world, c);
            }
        }
    }
    for c in board_corps(world) {
        let chair = board_of(world, c).and_then(|(_, ch)| ch);
        if chair.is_none_or(|ch| !law::living(world, ch)) {
            keep_up(world, c);
            vote(world, c, "the chair is dead");
            continue;
        }
        if tod == 0 {
            keep_up(world, c);
        }
        if tod != NOON {
            continue;
        }
        let due = last_vote(world, c) + Tick::from(world.config.governance.vote_days) * TICKS_PER_DAY;
        if world.tick >= due {
            vote(world, c, "the scheduled vote");
        } else if due - world.tick <= Tick::from(world.config.governance.campaign_days) * TICKS_PER_DAY {
            campaign(world, c);
        }
    }
    world.stats.current.gov.boards = board_corps(world).len() as u32;
}

fn last_vote(world: &World, corp: EntityId) -> Tick {
    match world.comp::<Corp>(corp).map(|c| &c.governance) {
        Some(Governance::Board { last_vote, .. }) => *last_vote,
        _ => world.tick,
    }
}

/// Drop the dead and the departed (the chair stays while it lives) and
/// fill to `board_size`.
fn keep_up(world: &mut World, corp: EntityId) {
    let Some((mut members, mut chair)) = board_of(world, corp) else { return };
    // An exec named outside the vote (a replacement, a god) takes the chair.
    let exec = world.comp::<Corp>(corp).and_then(|c| c.exec).filter(|&e| law::living(world, e));
    if let Some(e) = exec.filter(|&e| Some(e) != chair && chair.is_none_or(|c| !law::living(world, c))) {
        chair = Some(e);
        if !members.contains(&e) {
            members.push(e);
        }
        if let Some(Governance::Board { chair: c, .. }) = world.comp_mut::<Corp>(corp).map(|c| &mut c.governance) {
            *c = Some(e);
        }
    }
    let mut keep: Vec<EntityId> = members
        .into_iter()
        .filter(|&m| law::living(world, m) && (Some(m) == chair || employed_at(world, m, corp)))
        .collect();
    let size = world.config.governance.board_size.max(1);
    if keep.len() < size {
        let more = next_members(world, corp, &keep, size - keep.len());
        keep.extend(more);
    }
    keep.sort();
    if let Some(Governance::Board { members, .. }) = world.comp_mut::<Corp>(corp).map(|c| &mut c.governance) {
        *members = keep;
    }
}

/// `m`'s preference for `cand` (spec § 6); a member's own candidacy at
/// affinity 1.
pub fn pref(world: &World, m: EntityId, cand: EntityId) -> f32 {
    let aff = if m == cand { 1.0 } else { world.edge(m, cand).map_or(0.0, |e| e.affinity) };
    let r = reputation::rep(world, cand);
    0.4 * aff + 0.3 * r.standing + 0.3 * (r.honour - 0.5) * 2.0
}

/// The candidates: the living chair plus the members with `greed ×
/// standing ≥ ambition_min`; with none, the member of the highest standing.
pub fn candidates(world: &World, corp: EntityId) -> Vec<EntityId> {
    let Some((members, chair)) = board_of(world, corp) else { return Vec::new() };
    let min = world.config.governance.ambition_min;
    let mut c: Vec<EntityId> = members
        .iter()
        .copied()
        .filter(|&m| law::living(world, m))
        .filter(|&m| {
            Some(m) == chair || world.comp::<Personality>(m).map_or(0.0, |p| p.greed) * standing(world, m) >= min
        })
        .collect();
    if c.is_empty() {
        if let Some(&m) = members
            .iter()
            .filter(|&&m| law::living(world, m))
            .max_by(|&&a, &&b| standing(world, a).total_cmp(&standing(world, b)).then(b.cmp(&a)))
        {
            c.push(m);
        }
    }
    c
}

/// The candidate `m` prefers (ties the lower id).
fn choice(world: &World, m: EntityId, cands: &[EntityId]) -> Option<EntityId> {
    cands.iter().copied().max_by(|&a, &b| pref(world, m, a).total_cmp(&pref(world, m, b)).then(b.cmp(&a)))
}

/// Who `m`'s vote is pinned for on `corp`'s board: a campaign's Persuade,
/// else an active `Vote` coercion over it.
pub fn pinned_for(world: &World, corp: EntityId, m: EntityId) -> Option<EntityId> {
    if let Some(Governance::Board { pinned, .. }) = world.comp::<Corp>(corp).map(|c| &c.governance) {
        if let Some(&(_, f)) = pinned.iter().find(|p| p.0 == m) {
            return Some(f);
        }
    }
    world.coercions.values().filter(|c| c.is_active() && c.over == m).find_map(|c| match c.demand {
        Demand::Vote { corp: k, for_ } if k == corp => Some(for_),
        _ => None,
    })
}

/// Each member's ballot `(member, candidate, swung)`.
fn ballots(world: &World, corp: EntityId) -> Vec<(EntityId, EntityId, bool)> {
    let Some((members, _)) = board_of(world, corp) else { return Vec::new() };
    let cands = candidates(world, corp);
    let mut out = Vec::new();
    for m in members.into_iter().filter(|&m| law::living(world, m)) {
        let Some(free) = choice(world, m, &cands) else { continue };
        let pin = pinned_for(world, corp, m).filter(|f| cands.contains(f));
        match pin {
            Some(f) => out.push((m, f, f != free)),
            None => out.push((m, free, false)),
        }
    }
    out
}

/// The tally, most votes first (ties: the chair, then the lower id).
fn tally(world: &World, corp: EntityId, ballots: &[(EntityId, EntityId, bool)]) -> Vec<(EntityId, u32)> {
    let chair = board_of(world, corp).and_then(|(_, c)| c);
    let mut t: std::collections::BTreeMap<EntityId, u32> = std::collections::BTreeMap::new();
    for &(_, c, _) in ballots {
        *t.entry(c).or_default() += 1;
    }
    let mut v: Vec<(EntityId, u32)> = t.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then((Some(b.0) == chair).cmp(&(Some(a.0) == chair))).then(a.0.cmp(&b.0)));
    v
}

/// Hold the vote now: `VoteHeld` with the tally, a `VoteSwung` per pinned
/// member who voted against its own preference; a new chair is the exec.
/// The board's pins are spent and its `Vote` coercions complied.
pub fn vote(world: &mut World, corp: EntityId, why: &str) -> Option<EntityId> {
    let b = ballots(world, corp);
    if b.is_empty() {
        return None;
    }
    let t = tally(world, corp, &b);
    let winner = t.first().map(|&(w, _)| w);
    let old = board_of(world, corp).and_then(|(_, c)| c);
    let now = world.tick;
    for &(m, f, swung) in &b {
        if swung {
            world.stats.current.gov.votes_swung += 1;
            let text = format!(
                "{} voted for {} on {}'s board against their own preference",
                world.name_of(m),
                world.name_of(f),
                world.owner_label(Some(corp))
            );
            world.push_event(EventKind::VoteSwung, &[m, corp], text);
        }
    }
    let spent: Vec<crate::contract::CoercionId> = world
        .coercions
        .values()
        .filter(|c| c.is_active() && matches!(c.demand, Demand::Vote { corp: k, .. } if k == corp))
        .map(|c| c.id)
        .collect();
    for id in spent {
        leverage::finish(world, id, CoercionStatus::Complied, "the vote was cast");
    }
    let tally_text: Vec<String> = t.iter().map(|&(c, n)| format!("{} {n}", world.name_of(c))).collect();
    if let Some(Governance::Board { chair, last_vote, votes, pinned, .. }) =
        world.comp_mut::<Corp>(corp).map(|c| &mut c.governance)
    {
        *last_vote = now;
        *votes = b.iter().map(|&(m, c, _)| (m, c)).collect();
        pinned.clear();
        if winner.is_some() {
            *chair = winner;
        }
    }
    world.stats.current.gov.votes += 1;
    if let Some(w) = winner.filter(|&w| Some(w) != old) {
        world.stats.current.gov.chairs_fallen += u32::from(old.is_some_and(|o| law::living(world, o)));
        let at = world.comp::<crate::components::Job>(w).and_then(|j| j.employer);
        crate::systems::classpay::leave_for_promotion(world, w);
        crate::systems::classpay::note_promoted(world, w, at);
        if let Some(c) = world.comp_mut::<Corp>(corp) {
            c.exec = Some(w);
        }
        crate::systems::life::house_exec(world, w);
    }
    let text = format!(
        "{}'s board voted ({why}): {} · chair {}",
        world.owner_label(Some(corp)),
        tally_text.join(", "),
        winner.map_or("none".to_string(), |w| world.name_of(w))
    );
    world.push_event(EventKind::VoteHeld, &[corp, winner.unwrap_or(EntityId::NONE)], text);
    winner
}

/// Pin `m`'s vote on `corp`'s board for `for_` (a campaign's Persuade).
pub fn pin(world: &mut World, corp: EntityId, m: EntityId, for_: EntityId) {
    if let Some(Governance::Board { pinned, .. }) = world.comp_mut::<Corp>(corp).map(|c| &mut c.governance) {
        pinned.retain(|p| p.0 != m);
        pinned.push((m, for_));
    }
}

/// The board corp `a` and `b` both sit on (the lower id).
pub fn shared_board(world: &World, a: EntityId, b: EntityId) -> Option<EntityId> {
    board_corps(world).into_iter().find(|&c| board_of(world, c).is_some_and(|(m, _)| m.contains(&a) && m.contains(&b)))
}

/// The campaign's challenger and its swing member: the strongest
/// non-chair candidate losing by 1 or 2, and the member not voting for it
/// with the closest margin, not pinned for it, not tried this week.
pub fn campaign_target(world: &World, corp: EntityId) -> Option<(EntityId, EntityId)> {
    let (_, chair) = board_of(world, corp)?;
    let b = ballots(world, corp);
    let t = tally(world, corp, &b);
    let chair_votes = t.iter().find(|x| Some(x.0) == chair).map_or(0, |x| x.1);
    let cands = candidates(world, corp);
    let challenger = cands
        .iter()
        .copied()
        .filter(|&c| Some(c) != chair)
        .map(|c| (t.iter().find(|x| x.0 == c).map_or(0, |x| x.1), c))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))?;
    let (votes, ch) = challenger;
    if chair_votes <= votes || chair_votes - votes > 2 {
        return None;
    }
    b.iter()
        .filter(|&&(m, c, _)| c != ch && m != ch && !leverage::tried_recently(world, ch, m, leverage::tried::VOTE))
        .map(|&(m, c, _)| (pref(world, m, c) - pref(world, m, ch), m))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, m)| (ch, m))
}

/// One campaign step on `corp`'s board (spec § 6). A lawful challenger
/// makes the Persuade move with the bribe as its stake (no bribe in its
/// purse: its word); one that cannot talk (persuasion < 0.5) buys a
/// `Persuade` record instead when it can pay. An unlawful one blackmails
/// on a secret; else one that cannot threaten (intimidation < 0.5) buys a
/// `Vote` record (a regular leans for it), else threatens itself. Records
/// are brokered at the nearest open Fixer, else direct. True when tried.
pub fn campaign(world: &mut World, corp: EntityId) -> bool {
    let Some((ch, m)) = campaign_target(world, corp) else { return false };
    leverage::mark(world, ch, m, leverage::tried::VOTE);
    world.stats.current.gov.campaign_moves += 1;
    let lawful = world.comp::<Personality>(ch).map_or(0.5, |p| p.lawfulness) >= 0.5;
    let skills = world.comp::<crate::components::Skills>(ch).map_or((0.0, 0.0), |s| (s.persuasion, s.intimidation));
    let bribe = world.config.governance.vote_bribe;
    let broker = contracts::nearest_fixer(world, ch).filter(|&f| contracts::fixer_open(world, f));
    let target = Target::Agent(m);
    if lawful {
        let price = contracts::quote(world, ContractKind::Persuade, &target, broker);
        if skills.0 < 0.5 && world.purse(Some(ch)) >= price && post(world, ch, m, ContractKind::Persuade, broker) {
            world.stats.current.gov.board_persuades += 1;
            return true;
        }
        let paid = world.purse(Some(ch)) >= bribe;
        let stake = if paid { Stake::Coins(bribe) } else { Stake::Info { about: ch } };
        let mv = SocialMove { actor: ch, target: m, kind: MoveKind::Persuade, stake };
        if moves::resolve(world, &mv).success {
            if paid {
                ownership::pay(world, Some(ch), Some(m), bribe, Flow::Bribe);
            }
            pin(world, corp, m, ch);
        }
        return true;
    }
    if !leverage::on(world) {
        return true;
    }
    let demand = Demand::Vote { corp, for_: ch };
    if let Some(lev) = leverage::secret_on(world, ch, m) {
        if leverage::roll(world, ch, m, None, &lev, &demand).success {
            leverage::open(world, Some(ch), ch, m, lev, demand, CoercionWhy::Vote, None);
        }
        return true;
    }
    let price = contracts::quote(world, ContractKind::Vote, &target, broker);
    if skills.1 < 0.5 && world.purse(Some(ch)) >= price && post(world, ch, m, ContractKind::Vote, broker) {
        return true;
    }
    if leverage::roll(world, ch, m, None, &Leverage::Threat, &demand).success {
        leverage::open(world, Some(ch), ch, m, Leverage::Threat, demand, CoercionWhy::Vote, None);
    }
    true
}

fn post(world: &mut World, buyer: EntityId, m: EntityId, kind: ContractKind, broker: Option<EntityId>) -> bool {
    let deadline_days = world.config.contracts.deadline_days.get(kind);
    let p = Posting {
        buyer: Some(buyer),
        agent: Some(buyer),
        kind,
        target: Target::Agent(m),
        broker,
        deadline_days,
        origin: Origin::Board,
        price: None,
    };
    contracts::post(world, p).is_ok()
}

/// A board `Persuade` at its ledger due (`leverage::resolve_persuade`'s
/// Board arm): success pins the member for the buyer.
pub fn on_persuaded(world: &mut World, buyer: EntityId, m: EntityId) {
    if let Some(corp) = shared_board(world, buyer, m) {
        pin(world, corp, m, buyer);
    }
}

/// A `Vote` record at its ledger due: the taker's Intimidate on the
/// member (the leverage roll, a Threat with `Demand::Vote`); success opens
/// the coercion for the buyer and fulfils the record.
pub fn resolve_vote(world: &mut World, id: ContractId, taker: EntityId) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let Some(m) = c.target_agent() else { return };
    let Some(buyer) = c.buyer.or(c.agent) else { return };
    let corp = shared_board(world, buyer, m);
    let (Some(corp), true, true) = (corp, law::living(world, m), law::living(world, buyer)) else {
        contracts::settle(world, id, contracts::Settle::Cancelled, "the board is gone");
        return;
    };
    let demand = Demand::Vote { corp, for_: buyer };
    if leverage::roll(world, taker, m, None, &Leverage::Threat, &demand).success {
        leverage::open(world, Some(buyer), taker, m, Leverage::Threat, demand, CoercionWhy::Vote, None);
        contracts::settle(world, id, contracts::Settle::Fulfilled, "");
    } else {
        contracts::fail_attempt(world, id, "the member would not be leaned on");
    }
}

/// God `CallVote`: the vote now. Err for a corp without a board.
pub fn call_vote(world: &mut World, corp: EntityId) -> Result<EntityId, String> {
    if !world.has::<Corp>(corp) {
        return Err("not a corp".to_string());
    }
    if board_of(world, corp).is_none() && !seat(world, corp) {
        return Err("no board and no exec to seat one".to_string());
    }
    keep_up(world, corp);
    vote(world, corp, "called by god").ok_or_else(|| "nobody to vote".to_string())
}
