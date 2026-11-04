use super::*;

/// Branches that died or left drop off the list.
pub(super) fn prune(world: &mut World, id: OutsideId) {
    let Some(p) = parent(world, id) else { return };
    let keep: Vec<EntityId> = p
        .branches
        .iter()
        .copied()
        .filter(|&b| {
            world.is_alive(b)
                && (world.comp::<Corp>(b).is_some_and(|c| c.parent == Some(id))
                    || world.has::<Gang>(b)
                    || world.has::<Law>(b))
        })
        .collect();
    // A corp branch that died lost its component: a branch neither a gang
    // nor the Law was a corp.
    let had_corp = p.branches.iter().any(|&b| !world.has::<Gang>(b) && !world.has::<Law>(b));
    let has_corp = keep.iter().any(|&b| world.has::<Corp>(b));
    let now = world.tick;
    if let Some(p) = parent_mut(world, id) {
        p.branches = keep;
        if had_corp && !has_corp {
            p.branch_died_at = Some(now);
        }
    }
}

/// § 1 step 4: the branches send coins home. A corp branch whose closing
/// is at least `remit_floor × treasury_ref` sends `remit_share` of
/// yesterday's positive cashflow; a syndicate's gang, on the week's first
/// midnight, `tribute_share` of its treasury above `tribute_floor`. The
/// Treasury keeps the tariff's share (`Flow::Tax`); the rest crosses out
/// (`Flow::Remit`). Both are capital: neither is a trading loss. Nothing
/// under the city's Embargo.
pub fn remit(world: &mut World, id: OutsideId) {
    let Some(p) = parent(world, id) else { return };
    if p.embargoed {
        return;
    }
    let branches = p.branches.clone();
    let cfg = world.config.outside.parents.clone();
    let tariff = world.outside.tariff.clamp(0.0, 1.0);
    let weekly = world.day().is_multiple_of(7);
    let (mut sent, mut kept) = (0i64, 0i64);
    for b in branches {
        let amount = if let Some(c) = world.comp::<Corp>(b) {
            let cf = c.cashflow.back().copied().unwrap_or(0);
            if cf <= 0 || (c.closing as f64) < f64::from(cfg.remit_floor) * c.treasury_ref as f64 {
                continue;
            }
            ((f64::from(cfg.remit_share) * cf as f64).round() as i64).min(c.treasury.max(0))
        } else if let Some(g) = world.comp::<Gang>(b) {
            if !weekly {
                continue;
            }
            (f64::from(cfg.tribute_share) * (g.treasury - cfg.tribute_floor).max(0) as f64).round() as i64
        } else {
            continue;
        };
        if amount <= 0 {
            continue;
        }
        let cut = (f64::from(tariff) * amount as f64).round() as i64;
        let taxed = if cut > 0 { ownership::pay(world, Some(b), None, cut, Flow::Tax) } else { 0 };
        let moved = ownership::cross_out(world, Some(b), id, amount - taxed, Flow::Remit, false);
        if let Some(c) = world.comp_mut::<Corp>(b) {
            // Capital: the corp's day of trading is not the poorer for it.
            c.cashflow_today += taxed + moved;
        }
        sent += moved;
        kept += taxed;
    }
    if let Some(f) = world.outside.faction_mut(id) {
        // `cross_out` books an account's inflows as `income_today` (the
        // World's reading); a parent's income is its own (the ledger).
        f.income_today -= sent;
    }
    if let Some(p) = parent_mut(world, id) {
        p.ledger.remitted_today += sent;
        p.ledger.tariff_today += kept;
    }
}

/// A branch's `cash`: a corp's closing ÷ its `treasury_ref`, a gang's
/// treasury ÷ `gang_cash_ref`; the Law none.
fn cash_of(world: &World, b: EntityId) -> Option<(f32, i64, i64)> {
    if let Some(c) = world.comp::<Corp>(b) {
        let r = c.treasury_ref.max(1);
        return Some((c.closing as f32 / r as f32, c.closing, r));
    }
    if let Some(g) = world.comp::<Gang>(b) {
        // An emptied gang waits on its refound, not a subsidy.
        if g.emptied && g.members.is_empty() {
            return None;
        }
        let r = world.config.outside.parents.gang_cash_ref.max(1);
        return Some((g.treasury as f32 / r as f32, g.treasury, r));
    }
    None
}

/// § 2: the considerations table (phase 2: Hold and Subsidise), rescored
/// each midnight with hysteresis and a minimum hold, then the order's
/// action. The State holds (its orders are phase 3's).
pub fn decide(world: &mut World, id: OutsideId) {
    let Some(f) = world.outside.faction(id) else { return };
    let Some(p) = f.parent.as_ref() else { return };
    if f.kind == OutsideKind::State {
        return;
    }
    let cfg = world.config.outside.parents.clone();
    let weakest: Option<(EntityId, f32, i64, i64)> = p
        .branches
        .iter()
        .filter_map(|&b| cash_of(world, b).map(|(cash, closing, r)| (b, cash, closing, r)))
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let wealth = sat(f.treasury as f32 / f.treasury_ref.max(1) as f32);
    let att = (p.attitude + 1.0) / 2.0;
    let current = p.order;
    let h = |o: OutsideOrder| if o == current { cfg.hysteresis } else { 0.0 };
    let mut trace: Vec<(OutsideOrder, f32)> = vec![(OutsideOrder::Hold, cfg.flat_hold + h(OutsideOrder::Hold))];
    let cash = weakest.map_or(1.0, |w| w.1);
    let subsidise = [
        Consideration::raw(
            "a weak branch",
            cash,
            can(!p.embargoed
                && !state_sanctioned(world, id)
                && f.treasury > 0
                && weakest.is_some()
                && cash < cfg.subsidise_below),
        ),
        Consideration::new("1 - cash", 1.0 - cash, Curve::Linear { m: 0.8, b: 0.2 }),
        Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        Consideration::new("attitude", att, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("exposure", p.exposure, Curve::Linear { m: 0.5, b: 0.5 }),
    ];
    let mut push_scored = |order: OutsideOrder, cs: &[Consideration], flat: f32| {
        if cs.iter().all(|c| c.output > 0.0) {
            let raw: f32 = cs.iter().map(|c| c.output).product();
            trace.push((order, compensate(raw, cs.len()) + flat + h(order)));
        }
    };
    push_scored(OutsideOrder::Subsidise, &subsidise, cfg.flat_subsidise);
    // Phase 3 (§ 2's table): Reinforce, Retake, FoundBranch (megacorps),
    // Abandon (megacorps and syndicates).
    let now = world.tick;
    let persona = p.persona;
    let loss = p.branch_loss;
    let megacorp = f.kind == OutsideKind::Megacorp;
    let rival = if megacorp { rival_of(world, id) } else { None };
    let margin = {
        let n = p.ledger.income.len().min(7) as i64;
        let inc = sum_last(&p.ledger.income, 7);
        let up = f.base_upkeep.max(1) * n.max(1);
        if n == 0 {
            0.0
        } else {
            (inc - up) as f32 / up as f32
        }
    };
    let corp_branch = p.branches.iter().any(|&b| world.has::<Corp>(b));
    if megacorp {
        let recent_t = now.saturating_sub(u64::from(cfg.recent_days) * TICKS_PER_DAY);
        let recent = p.lost.iter().any(|l| l.at >= recent_t)
            || p.branches
                .iter()
                .any(|&b| world.comp::<Corp>(b).and_then(|c| c.raided_at).is_some_and(|t| t >= recent_t));
        let cooled = p
            .last_reinforce
            .is_none_or(|t| now.saturating_sub(t) >= u64::from(cfg.reinforce_cooldown_days) * TICKS_PER_DAY);
        let room = living_outsiders(world, id) < cfg.outsiders_cap as usize;
        let reinforce = [
            Consideration::raw(
                "a recent shock",
                0.0,
                can(recent
                    && cooled
                    && room
                    && corp_branch
                    && !p.embargoed
                    && !state_sanctioned(world, id)
                    && f.treasury >= (cfg.body_cost + cfg.body_fund) * i64::from(cfg.bodies_per_call)),
            ),
            Consideration::new("loss", loss, Curve::Logistic { k: 8.0, mid: 0.3 }),
            Consideration::new("pride", persona.pride, Curve::Linear { m: 0.6, b: 0.4 }),
            Consideration::new("attitude", att, Curve::Linear { m: 0.5, b: 0.5 }),
            Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        ];
        push_scored(OutsideOrder::Reinforce, &reinforce, cfg.flat_reinforce);
        let window_t = now.saturating_sub(u64::from(cfg.retake_window_days) * TICKS_PER_DAY);
        let target = p.lost.iter().any(|l| l.at >= window_t && retake_holder(world, id, l.building).is_some());
        let retake = [
            Consideration::raw(
                "a lost building",
                0.0,
                can(target && corp_branch && !p.embargoed && wealth >= cfg.retake_wealth_min),
            ),
            Consideration::new("loss", loss, Curve::Linear { m: 0.7, b: 0.3 }),
            Consideration::new("pride", persona.pride, Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 }),
            Consideration::new("exposure", p.exposure, Curve::Linear { m: 0.5, b: 0.5 }),
            Consideration::new("attitude", att, Curve::Linear { m: 0.6, b: 0.4 }),
        ];
        push_scored(OutsideOrder::Retake, &retake, cfg.flat_retake);
        let abandoned_ok = p.abandoned_until.is_none_or(|t| t <= now);
        let since = p.branch_died_at.map_or(0.0, |t| now.saturating_sub(t) as f32 / (14.0 * TICKS_PER_DAY as f32));
        let found = [
            Consideration::raw(
                "no branch",
                0.0,
                can(!corp_branch && abandoned_ok && !p.embargoed && f.treasury >= 3 * cfg.branch_seed),
            ),
            Consideration::new("exposure", p.exposure, Curve::Linear { m: 0.7, b: 0.3 }),
            Consideration::new("pride", persona.pride, Curve::Linear { m: 0.6, b: 0.4 }),
            Consideration::new("days without", since, Curve::Logistic { k: 8.0, mid: 0.5 }),
            Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
        ];
        push_scored(OutsideOrder::FoundBranch, &found, cfg.flat_found_branch);
        // Phase 4 (§ 3): Sanction and Campaign against the least-regarded rival.
        if let Some((r, regard)) = rival.filter(|_| corp_branch) {
            let ill = (1.0 - regard) / 2.0;
            let sanctioning = world.outside.sanctions.iter().any(|s| s.by == Some(id) && s.until > now);
            let sanction_cs = [
                Consideration::raw("a rival", 0.0, can(regard < cfg.sanction_regard && margin >= 0.0 && !sanctioning)),
                Consideration::new("1 - regard", ill, Curve::Linear { m: 0.7, b: 0.3 }),
                Consideration::new("pride", persona.pride, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("margin", margin.clamp(0.0, 1.0), Curve::Linear { m: 0.5, b: 0.5 }),
            ];
            push_scored(OutsideOrder::Sanction, &sanction_cs, cfg.flat_sanction);
            let campaign_cs = [
                // A disliked rival only (the plan's table opens it on any
                // rival; every healthy parent campaigned).
                Consideration::raw(
                    "a rival",
                    0.0,
                    can(regard < cfg.sanction_regard
                        && margin >= cfg.campaign_margin
                        && uplink_node(world, r).is_some()),
                ),
                Consideration::new("1 - regard", ill, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("1 - lawfulness", 1.0 - persona.lawfulness, Curve::Linear { m: 0.6, b: 0.4 }),
                Consideration::new("greed", persona.greed, Curve::Linear { m: 0.5, b: 0.5 }),
                Consideration::new("wealth", wealth, Curve::Linear { m: 0.5, b: 0.5 }),
            ];
            push_scored(OutsideOrder::Campaign, &campaign_cs, cfg.flat_campaign);
        }
    }
    let spent30 = sum_last(&p.ledger.spent, RING_DAYS);
    let burn = sat(spent30 as f32 / (30.0 * f.base_income.max(1) as f32));
    let abandon_cs = [
        Consideration::raw("a tired parent", 0.0, can(!p.branches.is_empty() && p.attitude < cfg.abandon_attitude)),
        Consideration::new("spent", burn, Curve::Logistic { k: 8.0, mid: 0.5 }),
        Consideration::new("greed", persona.greed, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("1 - exposure", 1.0 - p.exposure, Curve::Linear { m: 0.6, b: 0.4 }),
        Consideration::new("1 - attitude", 1.0 - att, Curve::Linear { m: 0.7, b: 0.3 }),
    ];
    push_scored(OutsideOrder::Abandon, &abandon_cs, cfg.flat_abandon);
    trace.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let best = trace[0].0;
    let pinned = p.pinned_until.is_some_and(|t| t > now);
    let held = !pinned && world.tick.saturating_sub(p.order_since) >= u64::from(cfg.min_hold_days) * TICKS_PER_DAY;
    // An order whose gate closed (no weak branch left) ends at once.
    let still = pinned || trace.iter().any(|t| t.0 == current);
    let next = if best != current && (held || !still) { best } else { current };
    let name = f.name.clone();
    if let Some(p) = parent_mut(world, id) {
        p.order_trace = trace;
        if next != current {
            p.order = next;
            p.order_since = now;
        }
        p.order_target = if next == OutsideOrder::Subsidise { weakest.map(|w| w.0) } else { None };
        if !pinned || p.rival.is_none() {
            p.rival = rival.map(|r| r.0);
        }
    }
    if next != current {
        world.push_event(EventKind::OutsideOrder, &[], format!("{name}: {} -> {}", current.label(), next.label()));
    }
    match next {
        OutsideOrder::Subsidise => {
            if let Some((b, _, closing, r)) = weakest {
                subsidise_branch(world, id, b, closing, r);
            }
        }
        OutsideOrder::Reinforce if next != current => {
            start_reinforce(world, id);
        }
        OutsideOrder::Retake => retake_step(world, id),
        OutsideOrder::FoundBranch => {
            found_branch(world, id);
        }
        OutsideOrder::Abandon if next != current => abandon(world, id),
        OutsideOrder::Sanction => {
            let mine = world.outside.sanctions.iter().any(|s| s.by == Some(id) && s.until > now);
            if let (Some(r), false) = (parent(world, id).and_then(|p| p.rival), mine) {
                let cfg = world.config.outside.parents.clone();
                sanction(world, Some(id), r, cfg.sanction_days, cfg.sanction_cut);
            }
        }
        OutsideOrder::Campaign => campaign_step(world, id),
        _ => {}
    }
}

/// Subsidise's daily action: `min(subsidy_cap_day, subsidise_to × ref −
/// closing, max_spend_frac × treasury)` into the weakest branch
/// (`Flow::ParentFund`, capital: a corp's cashflow does not count it).
fn subsidise_branch(world: &mut World, id: OutsideId, b: EntityId, closing: i64, r: i64) {
    let cfg = world.config.outside.parents.clone();
    let Some(f) = world.outside.faction(id) else { return };
    let to = (f64::from(cfg.subsidise_to) * r as f64).round() as i64 - closing;
    let spend = (f64::from(cfg.max_spend_frac) * f.treasury.max(0) as f64).round() as i64;
    let amount = cfg.subsidy_cap_day.min(to).min(spend);
    if amount <= 0 {
        return;
    }
    let moved = ownership::cross_in(world, id, Some(b), amount, Flow::ParentFund);
    if moved <= 0 {
        return;
    }
    if let Some(f) = world.outside.faction_mut(id) {
        f.income_today += moved;
    }
    if let Some(c) = world.comp_mut::<Corp>(b) {
        c.cashflow_today -= moved;
    }
    let pname = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    let bname = world.owner_label(Some(b));
    if let Some(p) = parent_mut(world, id) {
        p.ledger.spent_today += moved;
        p.ledger.subsidies_today += 1;
    }
    world.push_event(EventKind::Subsidised, &[b], format!("{pname} sent {moved} to {bname}"));
}

/// A branch's value: a corp's buildings at `ownership::value`, a gang's
/// members at `member_value`; the Law none.
fn branch_value(world: &World, b: EntityId) -> i64 {
    if let Some(c) = world.comp::<Corp>(b) {
        return c
            .buildings
            .iter()
            .filter_map(|&x| world.comp::<crate::components::Building>(x))
            .map(|bd| ownership::value(world, bd.kind))
            .sum();
    }
    if let Some(g) = world.comp::<Gang>(b) {
        return g.members.len() as i64 * world.config.outside.parents.member_value;
    }
    0
}

/// § 1 steps 1-3 and 5-8: today's counters into the rings; the market walk;
/// `branch_loss`; reputation; income abroad less upkeep (minted); health;
/// attitude; the failure flags.
pub(super) fn book_day(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let (seed, day, now) = (world.seed(), world.day(), world.tick);
    let gap = remit_gap(world, id);
    // Phase 4 (economic warfare): the districts its corp branches control.
    let held_districts = {
        let corps = corp_branches(world, id);
        world
            .districts
            .iter()
            .filter(|d| matches!(d.control, crate::components::Controller::Corp(c) if corps.contains(&c)))
            .count() as i64
    };
    let cut = sanction_cut(world, id);
    let sanction_costs: i64 =
        world.outside.sanctions.iter().filter(|s| s.by == Some(id) && s.until > now).count() as i64 * cfg.sanction_cost;
    let Some(f) = world.outside.faction(id) else { return };
    let Some(p) = f.parent.as_ref() else { return };
    // Phase 3: an emptied gang waiting on its refound is no lost value (the
    // refound reads the attitude).
    let live: Vec<EntityId> = p
        .branches
        .iter()
        .copied()
        .filter(|&b| world.comp::<Gang>(b).is_none_or(|g| !(g.emptied && g.members.is_empty())))
        .collect();
    let value: i64 = live.iter().map(|&b| branch_value(world, b)).sum();
    // Reputation's target from what the city knows of the branches (M15).
    let reps: Vec<crate::word::Reputation> =
        p.branches.iter().map(|&b| crate::systems::reputation::rep(world, b)).collect();
    let target = if reps.is_empty() {
        p.rep
    } else if f.kind == OutsideKind::State {
        reps.iter().map(|r| r.honour).sum::<f32>() / reps.len() as f32
    } else {
        let n = reps.len() as f32;
        0.5 * reps.iter().map(|r| r.honour).sum::<f32>() / n
            + 0.3 * reps.iter().map(|r| r.standing).sum::<f32>() / n
            + 0.2 * (1.0 - reps.iter().map(|r| r.heat).sum::<f32>() / n)
    };
    let z = crate::systems::econ::hash_normal(seed, PURPOSE_PARENT_MARKET, u64::from(id), day);
    let name = f.name.clone();
    let kind = f.kind;
    let Some(f) = world.outside.faction_mut(id) else { return };
    f.market = (1.0 + (f.market - 1.0) * (1.0 - cfg.market_revert) + cfg.market_sd * z).clamp(0.7, 1.3);
    let (market, base_income, upkeep) = (f.market, f.base_income, f.base_upkeep.max(1));
    let Some(p) = f.parent.as_mut() else { return };
    let l = &mut p.ledger;
    push(&mut l.remitted, std::mem::take(&mut l.remitted_today));
    push(&mut l.spent, std::mem::take(&mut l.spent_today));
    l.tariff_last = std::mem::take(&mut l.tariff_today);
    l.subsidies_last = std::mem::take(&mut l.subsidies_today);
    let all_waiting = live.is_empty() && kind == OutsideKind::Syndicate;
    if !all_waiting {
        push(&mut l.branch_value, value);
    }
    push(&mut l.districts, held_districts);
    let peak = l.branch_value.iter().copied().max().unwrap_or(0);
    let d_peak = l.districts.iter().copied().max().unwrap_or(0);
    // Phase 4: districts lost since the 30-day peak add 0.3 of their share.
    let d_term = if d_peak > 0 { 0.3 * (d_peak - held_districts).max(0) as f32 / d_peak as f32 } else { 0.0 };
    let loss =
        if kind == OutsideKind::State || all_waiting { 0.0 } else { (branch_loss(value, peak, gap) + d_term).min(1.0) };
    p.branch_loss = loss;
    p.rep = (p.rep + cfg.rep_rate * (target - p.rep)).clamp(0.0, 1.0);
    // Phase 4: the edge recovers while every track holds `data_floor`; Data
    // comes in from abroad.
    if p.data.units.iter().all(|&u| u >= cfg.data_floor) {
        p.edge += cfg.edge_recover * (1.0 - p.edge);
    }
    for (u, d) in p.data.units.iter_mut().zip(cfg.data_per_day) {
        *u += d;
    }
    p.edge = p.edge.clamp(cfg.edge_min, cfg.edge_max);
    let cut = 1.0 - (1.0 - cut) * if p.embargoed { 1.0 - cfg.embargo_cut } else { 1.0 };
    let income = parent_income(base_income, market, p.edge, p.rep, p.exposure, loss, cut);
    push(&mut p.ledger.income, income);
    let (inc7, n7) = (sum_last(&p.ledger.income, 7), p.ledger.income.len().min(7) as i64);
    let (rem7, spent7) = (sum_last(&p.ledger.remitted, 7), sum_last(&p.ledger.spent, 7));
    let delta = income - upkeep - sanction_costs;
    f.treasury += delta;
    f.income_today = income;
    let (treasury, tref) = (f.treasury, f.treasury_ref.max(1));
    let Some(p) = f.parent.as_mut() else { return };
    let week_upkeep = 7 * upkeep;
    let inc_week = if n7 > 0 { inc7 * 7 / n7 } else { income * 7 };
    let margin = (inc_week - week_upkeep) as f32 / week_upkeep as f32;
    p.health = ((0.5 + 2.0 * margin).clamp(0.0, 1.0) * (treasury as f32 / (0.25 * tref as f32)).clamp(0.0, 1.0))
        .clamp(0.0, 1.0);
    let remit_term = if p.exposure > 0.0 && base_income > 0 {
        cfg.att_remit * sat(rem7 as f32 / (7.0 * base_income as f32 * p.exposure))
    } else {
        0.0
    };
    let cost_term = if base_income > 0 { cfg.att_cost * sat(spent7 as f32 / (7.0 * base_income as f32)) } else { 0.0 };
    p.attitude = (p.attitude * (1.0 - cfg.att_decay) + remit_term - cost_term - cfg.att_loss * loss).clamp(-1.0, 1.0);
    // The State never fails (its income tracks its upkeep).
    let failing = n7 >= 7 && inc7 < week_upkeep && kind != OutsideKind::State;
    let newly_failing = failing && p.failing_since.is_none();
    p.failing_since = if failing { p.failing_since.or(Some(now)) } else { None };
    p.insolvent_since = if treasury < 0 { p.insolvent_since.or(Some(now)) } else { None };
    world.outside.minted += delta;
    if newly_failing {
        world.push_event(
            EventKind::ParentFailing,
            &[],
            format!("{name}: income {inc7} under its upkeep {week_upkeep} for a week"),
        );
    }
    // Phase 4: death by its creditors (income under upkeep `death_days`, or
    // a treasury under 0 `insolvent_days`). The State never dies.
    // Deviation: the creditors call in a failing parent only once its reserves
    // are under its `treasury_ref` (a parent fat on its branches' remittances
    // rides out a thin month: a founded parent, Nutrix Holdings, died on a
    // rich treasury and took the city's largest Food corp with it).
    let reserves = world.outside.faction(id).is_some_and(|f| f.treasury < f.treasury_ref);
    let Some(p) = parent(world, id) else { return };
    let failed =
        reserves && p.failing_since.is_some_and(|t| now.saturating_sub(t) >= u64::from(cfg.death_days) * TICKS_PER_DAY);
    let broke =
        p.insolvent_since.is_some_and(|t| now.saturating_sub(t) >= u64::from(cfg.insolvent_days) * TICKS_PER_DAY);
    if kind != OutsideKind::State && (failed || broke) {
        let _ = kill_parent(world, id, if failed { "its creditors called it in" } else { "insolvent" });
        return;
    }
    sync_uplink(world, id);
}

/// `Embargo`: the city's sanction on a parent while set.
pub fn set_embargo(world: &mut World, id: OutsideId, on: bool) -> Result<String, String> {
    let name = world.outside.faction(id).map(|f| f.name.clone());
    let Some(p) = parent_mut(world, id) else { return Err(format!("Embargo: no parent {id}")) };
    p.embargoed = on;
    let name = name.unwrap_or_default();
    Ok(if on { format!("Embargo on {name}") } else { format!("Embargo on {name} lifted") })
}

/// The day's parents group (`systems::stats::snapshot`, 23:59): the last
/// midnight's flows, today's crossings (every account), each parent's
/// state. `None` with the parents off.
pub fn close_day(world: &mut World) -> Option<crate::stats::OutsideCols> {
    if !parents_on(world) {
        return None;
    }
    let o = &world.outside;
    let mut cols = crate::stats::OutsideCols {
        crossed_in: o.inbound - o.inbound_mark,
        crossed_out: o.outbound - o.outbound_mark,
        ..Default::default()
    };
    let t = o.today.clone();
    cols.outsiders_living = o.outsiders.keys().filter(|&&a| world.is_alive(a)).count() as u32;
    cols.outsiders_arrived = t.arrived;
    cols.outsiders_left = t.left;
    cols.settled = t.settled;
    cols.reinforcements = t.reinforcements;
    cols.buybacks = t.buybacks;
    cols.retakes_posted = t.retakes_posted;
    cols.retakes_won = t.retakes_won;
    cols.branches_founded = t.branches_founded;
    cols.parents_founded = t.parents_founded;
    cols.abandoned = t.abandoned;
    cols.brothers_arrived = t.brothers;
    cols.gangs_refounded = t.refounded;
    cols.state_guards = u32::from(o.state_guards);
    cols.state_grants = t.grants;
    cols.mandates = t.mandates;
    cols.withdrawals = t.withdrawals;
    cols.uplink_runs = i64::from(t.uplink_runs);
    cols.uplink_breached = i64::from(t.uplink_breached);
    cols.data_shipped = i64::from(t.data_shipped);
    cols.sanctions_new = i64::from(t.sanctions);
    cols.sanctions_active = o.sanctions.iter().filter(|s| s.until > world.tick).count() as i64;
    cols.campaigns = i64::from(t.campaigns);
    cols.parents_dead = i64::from(t.parents_dead);
    cols.scrip_out = o.parents().filter_map(|f| f.parent.as_ref()).map(|p| p.scrip_out).sum();
    cols.scrip_paid = t.scrip_paid;
    cols.scrip_spent = t.scrip_spent;
    cols.scrip_converted = t.scrip_converted;
    cols.scrip_wiped = t.scrip_wiped;
    cols.parents_in = t.parents_in;
    cols.parents_out = t.parents_out;
    cols.successions = t.successions;
    cols.civil_wars = t.wars;
    cols.wars_active = world
        .gang_list()
        .iter()
        .filter(|&&g| world.comp::<crate::components::Gang>(g).is_some_and(|x| x.war.is_some()))
        .count() as u32;
    cols.war_loser_dead = t.war_dead;
    cols.war_loser_fled = t.war_fled;
    cols.war_loser_splinter = t.war_splinter;
    cols.war_loser_jailed = t.war_jailed;
    cols.abroad_posted = t.abroad_posted;
    cols.abroad_done = t.abroad_done;
    cols.abroad_won = t.abroad_won;
    cols.abroad_dead = t.abroad_dead;
    cols.away = o.away.len() as u32;
    cols.promoted = t.promoted;
    cols.cast_arrived = t.cast_arrived;
    cols.breaches = t.breaches;
    // The CSV's account slots are the seeded parents (the header is fixed;
    // a parent founded later is in the panel, not the CSV).
    let slots = usize::from(o.csv_parents);
    for f in o.parents().take(slots) {
        let Some(p) = f.parent.as_ref() else { continue };
        let l = &p.ledger;
        cols.remit += l.remitted.back().copied().unwrap_or(0);
        cols.parent_fund += l.spent.back().copied().unwrap_or(0);
        cols.failing += u32::from(p.failing_since.is_some());
        cols.accounts.push(crate::stats::ParentCols {
            treasury: f.treasury,
            income: l.income.back().copied().unwrap_or(0),
            health: p.health,
            attitude: p.attitude,
            rep: p.rep,
            branch_loss: p.branch_loss,
            order: p.order.label().to_string(),
            edge: p.edge,
        });
    }
    let (tariff, subsidies) = world
        .outside
        .factions
        .iter()
        .filter_map(|f| f.parent.as_ref())
        .fold((0, 0), |a, p| (a.0 + p.ledger.tariff_last, a.1 + p.ledger.subsidies_last));
    cols.tariff = tariff;
    cols.subsidies = subsidies;
    world.outside.today = Default::default();
    world.outside.inbound_mark = world.outside.inbound;
    world.outside.outbound_mark = world.outside.outbound;
    Some(cols)
}
