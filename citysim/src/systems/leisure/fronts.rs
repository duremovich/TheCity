use super::*;

// ---------------------------------------------------------------------------
// Fronts, the leader and the creed (plan L20)
// ---------------------------------------------------------------------------

/// A gang's standing fronts (owned by it, `front_of` it), ascending.
pub fn fronts_of(world: &World, gang: EntityId) -> Vec<EntityId> {
    // Jobs & lives phase 4: the Vice venues front too (none stands off).
    let mut out: Vec<EntityId> = [
        BuildingKind::FightPit,
        BuildingKind::Den,
        BuildingKind::Club,
        BuildingKind::StripClub,
        BuildingKind::Brothel,
        BuildingKind::Dollhouse,
    ]
    .iter()
    .flat_map(|&k| world.buildings_of_kind(k).iter().copied())
    .filter(|&b| {
        standing(world, b)
            && world
                .comp::<Building>(b)
                .is_some_and(|bd| bd.owner == Some(gang) && bd.venue.as_ref().is_some_and(|v| v.front_of == Some(gang)))
    })
    .collect();
    out.sort_unstable();
    out
}

/// Plan L20: the coins a gang keeps back from the daily stipends for the
/// leader's Collect (`tribute_share` of the week's tribute and take).
pub fn set_aside(world: &World, gang: EntityId) -> i64 {
    let take: i64 = fronts_of(world, gang)
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()))
        .map(|v| v.take_week.max(0))
        .sum();
    let week = world.comp::<Gang>(gang).map_or(0, |g| g.tribute_week.max(0)) + take;
    // Measured deviation: two days of its fronts' wages too (the stipends
    // left a front's Fighters and Croupiers unpaid; 18 unpaid quits).
    (world.config.leisure.tribute_share * week as f32).floor() as i64 + 2 * owner_payroll(world, Some(gang))
}

/// Plan L20: territory tribute credited today (`gang::daily_economy`).
pub fn note_tribute(world: &mut World, gang: EntityId, coins: i64) {
    if coins <= 0 {
        return;
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.tribute_week += coins;
    }
}

/// Plan L20, daily after the stipends: a Crackdown closes the gang's fronts
/// (in the districts it targets) for `crackdown_close_days`; a gang under
/// Expand with the treasury for a FightPit (a Sump district) or a Den
/// builds one, ≤ `fronts_max`, on a vacant Lot in a held district, else
/// refits a derelict there.
pub fn fronts_daily(world: &mut World, gang: EntityId) {
    // Fronts on the city's deed that still name this gang close too.
    let mut named: Vec<EntityId> = fronts_of(world, gang);
    for k in [
        BuildingKind::FightPit,
        BuildingKind::Den,
        BuildingKind::StripClub,
        BuildingKind::Brothel,
        BuildingKind::Dollhouse,
    ] {
        for &b in world.buildings_of_kind(k) {
            if standing(world, b)
                && !named.contains(&b)
                && world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).is_some_and(|v| v.front_of == Some(gang))
            {
                named.push(b);
            }
        }
    }
    let named_count = named.len() as u32;
    if crate::systems::law::cracking_down_on(world, gang) {
        use crate::components::{Posture, Stance};
        let city = world.law().is_some_and(|l| l.posture == Posture::Crackdown && l.target == Some(gang));
        let until = world.tick + world.config.leisure.crackdown_close_days * TICKS_PER_DAY;
        let now = world.tick;
        for b in named {
            let d = world.district_of_building(b);
            let hit = city || world.districts.get(d.index()).is_some_and(|x| x.stance == Stance::Crackdown(gang));
            if hit && !world.is_closed(b) {
                if let Some(bd) = world.comp_mut::<Building>(b) {
                    bd.closed_until = Some(until.max(now + 1));
                }
            }
        }
    }
    let cfg = world.config.leisure.clone();
    let Some(g) = world.comp::<Gang>(gang) else { return };
    // Review fix: the cap counts every front naming the gang, the seeded
    // ones still on the city's deed too.
    if g.order != crate::components::Order::Expand || named_count >= cfg.fronts_max || g.members.is_empty() {
        return;
    }
    let treasury = g.treasury;
    let held: Vec<DistrictId> = crate::systems::gang::held_districts(world, gang).into_iter().map(|(d, _)| d).collect();
    if held.is_empty() {
        return;
    }
    // Jobs & lives phase 4 (§ 5.1): with `[vice]` on a gang builds a
    // StripClub or a Brothel front too.
    // Jobs & lives phase 5 (§ 5.2): with the chip on, a Dollhouse front too.
    let kinds: &[BuildingKind] = if crate::systems::dollhouse::on(world) {
        &[
            BuildingKind::FightPit,
            BuildingKind::Den,
            BuildingKind::StripClub,
            BuildingKind::Brothel,
            BuildingKind::Dollhouse,
        ]
    } else if crate::systems::vice::on(world) {
        &[BuildingKind::FightPit, BuildingKind::Den, BuildingKind::StripClub, BuildingKind::Brothel]
    } else {
        &[BuildingKind::FightPit, BuildingKind::Den]
    };
    for &kind in kinds {
        let Some(cost) = crate::systems::founding::found_cost(world, kind) else { continue };
        // Measured deviation: and a week of the front's full staff's wages
        // on top (a front built on the found cost alone left its Fighters
        // unpaid: 14 unpaid quits on seed 43).
        let payroll = world.config.buildings.for_kind(kind).staff as i64
            * ownership::role_for(kind).map_or(0, |r| world.config.economy.wage(r));
        if treasury < cost + 7 * payroll {
            continue;
        }
        let lot = crate::systems::founding::vacant_lots(world)
            .into_iter()
            .filter_map(|l| {
                let bd = world.comp::<Building>(l)?;
                let tier = crate::systems::founding::door_tier(world, bd.door);
                let d = world.district_of_building(l);
                (crate::systems::founding::tier_ok(kind, tier) && held.contains(&d)).then_some(l)
            })
            .min();
        let built = match lot {
            Some(l) => match crate::systems::founding::build_on_lot(world, l, kind, Some(gang)) {
                Ok(b) => {
                    crate::systems::industry::pay_build(world, Some(gang), cost, Some(b), true);
                    Some(b)
                }
                Err(_) => None,
            },
            None => {
                let derelict = crate::systems::street::derelicts(world).into_iter().find(|&x| {
                    world.comp::<Building>(x).is_some_and(|bd| {
                        bd.kind == BuildingKind::Home
                            && crate::systems::founding::tier_ok(kind, bd.tier)
                            && held.contains(&world.district_of_building(x))
                    })
                });
                derelict.and_then(|x| crate::systems::founding::refit(world, x, kind, Some(gang)).ok())
            }
        };
        let Some(b) = built else { continue };
        let price = {
            let (tier, owner) = world.comp::<Building>(b).map_or((0, None), |bd| (bd.tier, bd.owner));
            crate::systems::jobs::venue_price(world, b, kind, tier, owner)
        };
        if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
            v.front_of = Some(gang);
            v.price = price;
        }
        let at =
            world.districts.get(world.district_of_building(b).index()).map_or("?", |x| x.name.as_str()).to_string();
        let text = format!("{} opened a {} front in {at}", world.owner_label(Some(gang)), kind.label());
        world.push_event(EventKind::Founded, &[gang, b], text);
        crate::systems::virt::relink(world);
        return;
    }
}

/// `stims::deal_bar` with leisure on: the gang's open front (or a Club in
/// its most-held district) nearest that district's centroid.
pub fn deal_venue(world: &World, gang: EntityId, d: DistrictId) -> Option<EntityId> {
    let centroid = world.district(d).centroid;
    let front = fronts_of(world, gang)
        .into_iter()
        .filter(|&b| open(world, b))
        .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(centroid), b)))
        .min()
        .map(|(_, b)| b);
    front.or_else(|| {
        world
            .buildings_of_kind(BuildingKind::Club)
            .iter()
            .copied()
            .filter(|&b| standing(world, b) && open(world, b) && world.district_of_building(b) == d)
            .filter_map(|b| world.comp::<Building>(b).map(|bd| (bd.door.manhattan(centroid), b)))
            .min()
            .map(|(_, b)| b)
    })
}

fn is_leader(world: &World, id: EntityId) -> Option<EntityId> {
    let gang = world.gang_of(id)?;
    world.comp::<Gang>(gang).filter(|g| g.leader == Some(id)).map(|_| gang)
}

/// L2 shadow fixes item 15: the start tick of the leader's running Lead
/// plan (`None` with the fixes off or no Lead under way).
fn lead_started(world: &World, gang: EntityId) -> Option<Tick> {
    let leader = world.comp::<Gang>(gang)?.leader?;
    world.comp::<Brain>(leader)?.plan.as_ref().filter(|p| p.goal == GoalKind::Lead).map(|p| p.started_tick)
}

/// Plan L20: the leader's Collect is due (the weekday, not done today).
/// L2 shadow fixes item 15: a round started on the collect day stays due
/// past midnight until its Collect runs.
pub(crate) fn collect_due(world: &World, gang: EntityId) -> bool {
    // Pressure pass F2 (OPEN-118's "Collected 0 to 0 members"): a week with
    // nothing to share is no Collect; the tribute waits for the next one.
    if collect_pool(world, gang) <= 0 {
        return false;
    }
    let today = world.day();
    let weekday = world.config.leisure.collect_weekday % 7;
    if let Some(t) = lead_started(world, gang) {
        let day = t / TICKS_PER_DAY;
        if day % 7 == weekday && world.comp::<Gang>(gang).is_some_and(|g| g.collected_day.is_none_or(|c| c < day)) {
            return true;
        }
    }
    today % 7 == weekday && world.comp::<Gang>(gang).is_some_and(|g| g.collected_day != Some(today))
}

/// Plan L20: a Call is due the evening after an order change. L2 shadow
/// fixes item 15: it stays due for a Lead plan's life (the leader walked
/// 330 min to the Hideout and dropped the Lead on the doorstep at night).
pub(crate) fn call_due(world: &World, gang: EntityId) -> bool {
    let Some(g) = world.comp::<Gang>(gang) else { return false };
    let pending = g.order_since > 0
        && g.called_for != Some(g.order_since)
        && world.tick.saturating_sub(g.order_since) < TICKS_PER_DAY;
    pending && (world.phase() == DayPhase::Evening || lead_started(world, gang).is_some())
}

/// Plan L20: the leader's `Lead` gate (`None`: not the leader, or nothing due).
pub fn lead_considerations(world: &World, id: EntityId) -> Option<(Vec<Consideration>, f32)> {
    if world.has::<Sentence>(id) {
        return None;
    }
    // The pre-view pass phase 6 (P14): a lieutenant sent for to court.
    if crate::systems::leaders::attend_due(world, id) {
        return Some((vec![Consideration::raw("sent for to court", 1.0, 0.6)], 0.0));
    }
    let gang = is_leader(world, id)?;
    // The pre-view pass phase 6 (P13-P15): the round, court, an order to give.
    let due = collect_due(world, gang) || call_due(world, gang) || crate::systems::leaders::due(world, gang, id);
    if !due {
        return None;
    }
    Some((vec![Consideration::raw("collect or call", 1.0, 0.6)], 0.0))
}

/// `already_satisfied(Lead)`.
pub fn lead_satisfied(world: &World, id: EntityId) -> bool {
    // Pressure pass round 9, Y2 (`OPEN-250`): a leader mourning or lying low
    // keeps to his Home: no round, no order walked, no court (V17
    // `fixer_23959 d23 09:19`: "lies low at home", then 875 min to collect).
    if crate::systems::grief::holed_up(world, id) {
        return true;
    }
    if crate::systems::leaders::attend_due(world, id) {
        return false;
    }
    is_leader(world, id)
        .is_none_or(|g| !collect_due(world, g) && !call_due(world, g) && !crate::systems::leaders::due(world, g, id))
}

/// Plan L20: the round: `GoTo(front) → Collect` per front on a collect day,
/// then `GoTo(Hideout) → Collect` (the pay-out and the Call).
pub fn lead_plan(world: &mut World, id: EntityId) -> Option<Plan> {
    // The pre-view pass phase 6 (P14): a lieutenant's walk to court.
    if crate::systems::leaders::attend_due(world, id) {
        return crate::systems::leaders::attend_plan(world, id);
    }
    let gang = is_leader(world, id)?;
    let hideout = world.hideout_of(gang)?;
    // The pre-view pass phase 6 (P13-P15): an order walked to a lieutenant,
    // court, or the round, whichever is due (the Collect and Call ride along).
    if let Some(plan) = crate::systems::leaders::plan(world, id, gang, hideout) {
        return Some(plan);
    }
    let mut steps = Vec::new();
    if collect_due(world, gang) {
        for f in fronts_of(world, gang) {
            steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(f)));
            steps.push(step(ActionKind::Collect, Some(f)));
        }
    } else if !call_due(world, gang) {
        return None;
    }
    steps.push(step(ActionKind::GoTo(LocationKey::Seller), Some(hideout)));
    steps.push(step(ActionKind::Collect, Some(hideout)));
    Some(Plan { goal: GoalKind::Lead, target: Some(hideout), steps, started_tick: world.tick })
}

/// A `Collect` step: at a front the week is booked (the coins reached the
/// treasury at each payment); at the Hideout the Call and the pay-out.
pub(super) fn collect_step(world: &mut World, id: EntityId, target: Option<EntityId>) {
    let Some(gang) = is_leader(world, id) else { return };
    if target.is_none() || target != world.hideout_of(gang) {
        // The pre-view pass phase 6 (P13): a round stop collects in person.
        if let Some(b) = target {
            crate::systems::leaders::round_stop(world, id, gang, b);
        }
        return;
    }
    if call_due(world, gang) {
        call(world, gang, id);
    }
    if collect_due(world, gang) {
        payout(world, gang, Some(id));
    }
}

pub(super) fn call(world: &mut World, gang: EntityId, leader: EntityId) {
    let until = world.tick + world.config.leisure.call_hours * TICKS_PER_HOUR;
    let Some(g) = world.comp_mut::<Gang>(gang) else { return };
    g.call_until = Some(until);
    g.called_for = Some(g.order_since);
    let text = format!("{} called {} to the Hideout", world.name_of(leader), world.owner_label(Some(gang)));
    world.push_event(EventKind::Called, &[leader, gang], text);
}

/// Plan L20: the coins a Collect shares, `tribute_share × (the week's
/// tribute + the fronts' take)`, net of the fronts' payroll the stipends
/// keep back (review fix), at most the treasury.
fn collect_pool(world: &World, gang: EntityId) -> i64 {
    let take: i64 = fronts_of(world, gang)
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()))
        .map(|v| v.take_week.max(0))
        .sum();
    let Some(g) = world.comp::<Gang>(gang) else { return 0 };
    let week = g.tribute_week.max(0) + take;
    let pool = (world.config.leisure.tribute_share * week as f32).floor() as i64;
    let keep = 2 * owner_payroll(world, Some(gang));
    pool.min((g.treasury - keep).max(0))
}

/// Plan L20: `tribute_share × (the week's territory tribute + the fronts'
/// take)` from the treasury to the members, equal shares, highest rank
/// first (the leader's ranking, then the rest by id); `Flow::Tribute`.
/// Resets the week. Returns the members paid.
pub fn payout(world: &mut World, gang: EntityId, leader: Option<EntityId>) -> u32 {
    let today = world.day();
    let fronts = fronts_of(world, gang);
    let take: i64 = fronts
        .iter()
        .filter_map(|&b| world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()))
        .map(|v| v.take_week.max(0))
        .sum();
    let Some(g) = world.comp::<Gang>(gang) else { return 0 };
    let week = g.tribute_week.max(0) + take;
    let pool = collect_pool(world, gang);
    let mut members: Vec<EntityId> = crate::systems::gang::leader_ranking(world, gang);
    let mut rest: Vec<EntityId> = g.members.iter().copied().filter(|m| !members.contains(m)).collect();
    rest.sort_unstable();
    members.extend(rest);
    let mut paid_members = 0u32;
    let mut paid_total = 0i64;
    // Lives at work phase 6 (§ 4.5): the cut off the top, the leader's
    // `leader_cut` and the lieutenants' `lieutenant_cut` shared, before the
    // equal shares (none off).
    let mut pool = pool;
    let mut cut_note = String::new();
    if pool > 0 && crate::systems::careers::lieutenants_on(world) {
        let lts: Vec<EntityId> = members
            .iter()
            .copied()
            .filter(|&m| world.comp::<crate::components::GangMember>(m).is_some_and(|x| x.rank == 1))
            .collect();
        let boss = world.comp::<Gang>(gang).and_then(|g| g.leader).filter(|l| members.contains(l));
        let base = pool;
        let (mut to_leader, mut to_lts) = (0i64, 0i64);
        if let Some(l) = boss {
            let c = (world.config.gangs.leader_cut * base as f32).floor() as i64;
            to_leader = ownership::pay(world, Some(gang), Some(l), c.min(pool), Flow::Tribute);
            pool -= to_leader;
        }
        if !lts.is_empty() {
            let c = ((world.config.gangs.lieutenant_cut * base as f32).floor() as i64).min(pool);
            let (share, extra) = (c / lts.len() as i64, c % lts.len() as i64);
            for (i, &m) in lts.iter().enumerate() {
                let amount = share + i64::from((i as i64) < extra);
                if amount > 0 {
                    to_lts += ownership::pay(world, Some(gang), Some(m), amount, Flow::Tribute);
                }
            }
            pool -= to_lts;
        }
        if to_leader > 0 || to_lts > 0 {
            let row = ownership::jobs2_row(world);
            row.leader_cut_coins += to_leader;
            row.lieutenant_cut_coins += to_lts;
            let who = boss.map_or_else(|| "the leader".to_string(), |l| world.name_of(l));
            cut_note = format!("; off the top {to_leader} to {who}, {to_lts} to {} lieutenants", lts.len());
        }
    }
    if pool > 0 && !members.is_empty() {
        let n = members.len() as i64;
        let share = pool / n;
        let extra = pool % n;
        for (i, m) in members.iter().enumerate() {
            let amount = share + i64::from((i as i64) < extra);
            if amount <= 0 {
                continue;
            }
            let p = ownership::pay(world, Some(gang), Some(*m), amount, Flow::Tribute);
            if p > 0 {
                crate::systems::gang::note_paid(world, *m);
                paid_members += 1;
                paid_total += p;
            }
        }
    }
    for b in fronts {
        if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
            v.take_week = 0;
        }
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.tribute_week = 0;
        g.collected_day = Some(today);
    }
    world.stats.current.living.collected += paid_members;
    // Pressure pass F2 (OPEN-118): an empty Collect (the pool spent between
    // the plan and the Hideout) closes the week without a line.
    if paid_total == 0 && cut_note.is_empty() {
        return 0;
    }
    let who = leader.map_or_else(|| "the leader".to_string(), |l| world.name_of(l));
    let text = format!(
        "{who} collected for {}: {paid_total} to {paid_members} members (the week's tribute and take {week}{cut_note})",
        world.owner_label(Some(gang))
    );
    let mut actors: Vec<EntityId> = leader.into_iter().collect();
    actors.push(gang);
    world.push_event(EventKind::Collected, &actors, text);
    paid_members
}

/// Plan L20: a leader who is not a body collects (and calls) without the
/// walk, from `living::run`'s 18:00 hour.
pub fn stat_leaders(world: &mut World) {
    for gang in world.gangs() {
        let Some(leader) = world.comp::<Gang>(gang).and_then(|g| g.leader) else { continue };
        let body = world.comp::<Brain>(leader).is_some_and(|b| b.lod != Lod::Statistical);
        if body && !world.has::<Sentence>(leader) {
            continue;
        }
        if call_due(world, gang) {
            call(world, gang, leader);
        }
        if collect_due(world, gang) {
            payout(world, gang, Some(leader));
        }
    }
}
