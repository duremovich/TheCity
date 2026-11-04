use super::*;

/// Jobs & lives: the day row's group, created on its first flow (only
/// with `[jobs2]` on; `systems::stats::snapshot` makes it every day then).
pub fn jobs2_row(world: &mut World) -> &mut crate::stats::Jobs2Cols {
    world.stats.current.jobs2.get_or_insert_with(Default::default)
}

pub(super) fn ledger(world: &mut World, flow: Flow, coins: i64) {
    let row = &mut world.stats.current;
    match flow {
        Flow::Food => row.flow_food += coins,
        Flow::Drink => row.flow_drink += coins,
        Flow::Wage | Flow::ExecWage => row.flow_wages += coins,
        Flow::Rent => row.flow_rent += coins,
        Flow::Upkeep => row.flow_upkeep += coins,
        Flow::Wholesale => row.flow_wholesale += coins,
        Flow::Contract => row.flow_contract += coins,
        Flow::Tax => row.flow_tax += coins,
        Flow::Found
        | Flow::Sale
        | Flow::Bribe
        | Flow::Subsidy
        | Flow::SellFood
        | Flow::JailFood
        | Flow::Hotel
        | Flow::Sanitation
        | Flow::Scavenge
        | Flow::Robbery => row.flow_other += coins,
        // Jobs v2 (P5a, J34): fines stay in `flow_other` and have a column.
        Flow::Fine => {
            row.flow_other += coins;
            row.jobs.flow_fines += coins;
        }
        Flow::Asset => row.flow_asset += coins,
        Flow::AssetUpkeep => row.flow_asset_upkeep += coins,
        Flow::Finance => row.flow_finance += coins,
        Flow::Import => row.flow_import += coins,
        Flow::Stims | Flow::StimsLegal => row.flow_stims += coins,
        Flow::Parts => row.flow_parts += coins,
        Flow::Treatment => row.flow_treatment += coins,
        Flow::Data => row.virt.flow_data += coins,
        Flow::Hack => row.virt.flow_hack += coins,
        Flow::IceUpkeep => row.virt.flow_ice_upkeep += coins,
        Flow::Research => row.virt.flow_research += coins,
        Flow::Terminal => row.virt.flow_terminal += coins,
        Flow::Ads => row.word.flow_ads += coins,
        Flow::Plant => row.word.flow_plant += coins,
        // L2 L12, L34: `flow_gamble` is net to houses (losses - wins).
        Flow::Leisure => row.living.flow_leisure += coins,
        Flow::Gamble => row.living.flow_gamble += coins,
        Flow::GambleWin => {
            row.living.flow_gamble -= coins;
            row.living.flow_gamble_win += coins;
        }
        Flow::Tribute => row.living.flow_tribute += coins,
        Flow::Export => row.living.flow_export += coins,
        Flow::StreetDice => row.living.flow_street_dice += coins,
        // M16a (plan C38).
        Flow::Escrow => row.contract.flow_escrow += coins,
        Flow::Payout => row.contract.flow_payout += coins,
        Flow::FixerCut => row.contract.flow_fixer_cut += coins,
        // Real economy (plan E45).
        Flow::Customs => row.econ.flow_customs += coins,
        Flow::Migrant => row.econ.flow_migrant_in += coins,
        Flow::Fence => row.econ.flow_fence += coins,
        // Real economy phase 3b/3c (E27, E28, E39).
        Flow::Donate => row.econ.flow_donate += coins,
        Flow::Alms => row.econ.flow_alms += coins,
        Flow::CampFood => row.econ.flow_camp_food += coins,
        Flow::Property => row.econ.flow_property += coins,
        Flow::Inputs => row.econ.flow_inputs += coins,
        Flow::Pool => {}
        Flow::Fare => row.transit.fares += coins,
        Flow::Coerced => row.leverage.flow_coerced += coins,
        Flow::Trauma => row.wounded.flow_trauma += coins,
        Flow::AmbientSale | Flow::Remit | Flow::ParentFund | Flow::Grant | Flow::Scrip => {}
        // Jobs & lives phase 1: the columns of its own group.
        Flow::Handling => row.jobs2.get_or_insert_with(Default::default).flow_handling += coins,
        Flow::Power => row.jobs2.get_or_insert_with(Default::default).flow_power += coins,
        Flow::Build => row.jobs2.get_or_insert_with(Default::default).flow_build += coins,
        // The later phases' flows: `flow_other` until their columns land.
        // Jobs & lives phase 3: the weapons group's own columns.
        Flow::Licence => row.jobs2.get_or_insert_with(Default::default).flow_licence += coins,
        Flow::Arms => row.jobs2.get_or_insert_with(Default::default).flow_arms += coins,
        // Jobs & lives phase 4: the tips' column.
        Flow::Tip => row.jobs2.get_or_insert_with(Default::default).tips += coins,
        // Jobs & lives phase 6: the tithes' column.
        Flow::Tithe => row.jobs2.get_or_insert_with(Default::default).tithes += coins,
        // Lives at work phase 1: the madam's cut.
        Flow::MadamCut => row.jobs2.get_or_insert_with(Default::default).flow_madam_cut += coins,
        // Lives at work phase 3: the Camp buy-out (the `Fine` shape).
        Flow::Buyout => {
            row.flow_other += coins;
            row.jobs2.get_or_insert_with(Default::default).flow_buyout += coins;
        }
        Flow::CabFare | Flow::StreetCut | Flow::Gray | Flow::Purse => row.flow_other += coins,
        // Round 6 V4: the fare's second hand (driver -> house) is no new flow of coins.
        Flow::CabTake => {}
        // The city milestone C6b: the floor rent's column.
        Flow::FloorRent => row.jobs2.get_or_insert_with(Default::default).flow_floor_rent += coins,
    }
}

/// The ledger line of a crossing out of the city (`cross_out`): the flows
/// with an outbound column of their own, else `ledger`.
fn ledger_out(world: &mut World, flow: Flow, coins: i64) {
    let row = &mut world.stats.current;
    match flow {
        Flow::Migrant => row.econ.flow_migrant_out += coins,
        Flow::Import => {
            row.flow_import += coins;
            row.econ.flow_import_out += coins;
        }
        other => ledger(world, other, coins),
    }
}

/// M14 V26: a flow that moves no coins (the city paying itself, the
/// Treasury's own nodes' ICE upkeep) still shows in its ledger column.
pub fn ledger_only(world: &mut World, flow: Flow, coins: i64) {
    if coins > 0 {
        ledger(world, flow, coins);
    }
}

/// Whole coins of tax on `amount` paid to `payee`, the fraction carried in
/// `World::tax_accum`.
fn withhold(world: &mut World, payee: EntityId, amount: i64) -> i64 {
    let rate = world.levers.tax_rate;
    if rate <= 0.0 {
        return 0;
    }
    let acc = world.tax_accum.entry(payee).or_insert(0.0);
    *acc += amount as f32 * rate;
    let tax = (acc.floor() as i64).clamp(0, amount);
    *acc -= tax as f32;
    tax
}

/// The `shadow` tool's flow note (L2 shadow V2): a no-op unless the log is on
/// and an end of the transfer is an agent.
fn shadow_flow(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, coins: i64, flow: Flow, refund: bool) {
    if world.shadow_notes.is_none() {
        return;
    }
    let agent = |e: Option<EntityId>| e.is_some_and(|e| world.has::<crate::components::Identity>(e));
    if !(agent(from) || agent(to)) {
        return;
    }
    let tick = world.tick;
    if let Some(log) = world.shadow_notes.as_mut() {
        log.push(crate::word::ShadowNote::Flow { tick, from, to, coins, flow, refund });
    }
}

pub(super) fn transfer(
    world: &mut World,
    from: Option<EntityId>,
    to: Option<EntityId>,
    amount: i64,
    flow: Flow,
    full: bool,
) -> i64 {
    if amount <= 0 {
        return 0;
    }
    // M17 phase 4: food or rent to an issuer's branch is paid in its scrip first.
    if matches!(flow, Flow::Food | Flow::Rent) && !world.outside.scrip.is_empty() {
        crate::systems::parents::scrip_spend(world, from, to, amount);
    }
    let moved = if full { amount } else { amount.min(world.purse(from).max(0)) };
    if moved <= 0 || from == to {
        // Paying oneself moves nothing and is no flow.
        return moved.max(0);
    }
    ledger(world, flow, moved);
    shadow_flow(world, from, to, moved, flow, false);
    // Round 6 V4 (OPEN-240): a rider's fare by payee kind (diagnostic).
    if flow == Flow::CabFare && from.is_some_and(|f| world.has::<crate::components::Identity>(f)) {
        let driver = to.is_some_and(|t| {
            world.jobs2.pitches.contains_key(&t)
                || world.comp::<crate::components::Job>(t).is_some_and(|j| {
                    crate::systems::trades::row_of(world, j.role)
                        .is_some_and(|r| r.duty == crate::config::TradeDuty::Fare)
                })
        });
        let row = jobs2_row(world);
        if driver {
            row.cab_paid_driver += 1;
        } else {
            row.cab_paid_other += 1;
        }
    }
    // The Treasury's census (diagnostic, never read by the simulation).
    let city = |w: &World, o: Option<EntityId>| matches!(owner_kind(w, o), OwnerKind::City);
    let label = |o: Option<EntityId>| if o.is_some() { format!("{flow:?} (dead owner)") } else { format!("{flow:?}") };
    if city(world, from) {
        *world.probe.treasury_out.entry(label(from)).or_insert(0) += moved;
    }
    let to_city = city(world, to);
    // The pressure pass phase 5b: the gangs' books by flow (diagnostic).
    let gang_kind = |w: &World, o: Option<EntityId>| o.is_some_and(|g| w.has::<Gang>(g));
    if gang_kind(world, from) {
        let to_kind = match owner_kind(world, to) {
            OwnerKind::City => "city",
            OwnerKind::Corp(_) => "corp",
            OwnerKind::Gang(_) => "gang",
            OwnerKind::Agent(_) => "agent",
        };
        *world.probe.gang_out.entry(format!("{flow:?} -> {to_kind}")).or_insert(0) += moved;
        if let Some(g) = from {
            *world.probe.gang_out_by.entry((g, format!("{flow:?} -> {to_kind}"))).or_insert(0) += moved;
        }
    }
    if gang_kind(world, to) {
        *world.probe.gang_in.entry(format!("{flow:?}")).or_insert(0) += moved;
        if let Some(g) = to {
            *world.probe.gang_in_by.entry(g).or_insert(0) += moved;
        }
    }
    world.purse_add(from, -moved);
    let tax = match to {
        Some(payee) if flow.taxed() => withhold(world, payee, moved),
        _ => 0,
    };
    world.purse_add(to, moved - tax);
    if to_city {
        *world.probe.treasury_in.entry(label(to)).or_insert(0) += moved - tax;
    }
    if tax > 0 {
        world.purse_add(None, tax);
        world.stats.current.flow_tax += tax;
        *world.probe.treasury_in.entry(format!("{flow:?} tax withheld")).or_insert(0) += tax;
    }
    // M13 D38: what reaches a gang's purse is gang income (dealing apart).
    if to.is_some_and(|g| world.config.assets.enabled && world.has::<Gang>(g)) {
        world.stats.current.gang_income += moved - tax;
        if flow == Flow::Stims {
            world.stats.current.gang_income_dealing += moved - tax;
        }
    }
    // Jobs v2 (P5a, J34): the Treasury's payroll (`civic_payroll`).
    if from.is_none() && flow == Flow::Wage {
        world.stats.current.jobs.civic_payroll += moved;
    }
    if flow.capital() {
        uncount_cashflow(world, from, moved);
        uncount_cashflow(world, to, -(moved - tax));
    }
    // Real economy phase 2 (plan E16, E45): a corp payee's revenue window
    // (net of the withheld tax; capital flows never count), and the
    // population's other inflows (wages on only).
    if crate::systems::wages::on(world) {
        note_revenue(world, to, flow, moved - tax);
        crate::systems::wages::note_inflow(world, to, flow, moved - tax);
    }
    // M17 phase 4: a parented corp branch's wage is part scrip.
    if flow == Flow::Wage {
        crate::systems::parents::scrip_wage(world, from, to, moved - tax);
    }
    moved
}

/// Plan E16: a taxed flow reaching a corp is revenue (`Corp.rev_today`).
pub(super) fn note_revenue(world: &mut World, to: Option<EntityId>, flow: Flow, net: i64) {
    if !(flow.taxed() || flow == Flow::CabTake) || net <= 0 {
        return;
    }
    if let Some(c) = to.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.rev_today += net;
    }
}

/// Plan E16: a corp's gross payroll today (`Corp.pay_today`, wages on only).
/// Class pay P3: the premium part of a graded wage (out of capital and
/// profit, not the labour pool), `Corp.prem_today`.
pub fn note_premium(world: &mut World, payer: Option<EntityId>, premium: i64) {
    if premium <= 0 || !crate::systems::classpay::on(world) {
        return;
    }
    if let Some(c) = payer.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.prem_today += premium;
    }
}

pub fn note_payroll(world: &mut World, payer: Option<EntityId>, gross: i64) {
    if gross <= 0 || !crate::systems::wages::on(world) {
        return;
    }
    if let Some(c) = payer.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.pay_today += gross;
    }
}

/// Take a capital transfer back out of a corp's `cashflow_today`.
fn uncount_cashflow(world: &mut World, owner: Option<EntityId>, delta: i64) {
    if let Some(c) = owner.and_then(|o| world.comp_mut::<Corp>(o)) {
        c.cashflow_today += delta;
    }
}

/// Move `min(amount, max(purse(from), 0))` from one purse to another: nobody
/// pays from a negative purse. Returns the coins moved.
pub fn pay(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, amount: i64, flow: Flow) -> i64 {
    transfer(world, from, to, amount, flow, false)
}

/// As `pay`, but a corp or the city pays the full amount and may go
/// negative (upkeep, the city's purchases); agents and gangs are capped.
pub fn charge(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, amount: i64, flow: Flow) -> i64 {
    let full = matches!(owner_kind(world, from), OwnerKind::City | OwnerKind::Corp(_));
    transfer(world, from, to, amount, flow, full)
}

/// Undo a `pay(agent -> owner, amount, flow)` (a purchase whose stock was
/// gone): the owner returns its share and the Treasury the tax it took. The
/// tax comes back through the payee's accumulator (`withhold` run
/// backwards), so the owner and the Treasury end where they started, not a
/// coin of rounding apart. Conserves coins; the owner may dip below zero.
pub fn refund(world: &mut World, agent: EntityId, owner: Option<EntityId>, amount: i64, flow: Flow) {
    if amount <= 0 || owner == Some(agent) {
        return;
    }
    ledger(world, flow, -amount);
    shadow_flow(world, owner, Some(agent), amount, flow, true);
    let rate = world.levers.tax_rate;
    let tax = match owner {
        Some(payee) if flow.taxed() && rate > 0.0 => {
            let acc = world.tax_accum.entry(payee).or_insert(0.0);
            *acc -= amount as f32 * rate;
            let back = if *acc < 0.0 { ((-*acc).ceil() as i64).clamp(0, amount) } else { 0 };
            *acc += back as f32;
            back
        }
        _ => 0,
    };
    world.purse_add(owner, -(amount - tax));
    if tax > 0 {
        world.purse_add(None, -tax);
        world.stats.current.flow_tax -= tax;
    }
    world.purse_add(Some(agent), amount);
}

/// Undo a `pay(owner -> Treasury, amount, Flow::Tax)` surcharge (the
/// `SetLeisureTax` cut on an abandoned purchase or a refunded bet): the
/// Treasury returns `amount` to `owner` and `flow_tax` falls by it.
/// Conserves coins; the Treasury may dip below zero.
pub fn refund_tax(world: &mut World, owner: Option<EntityId>, amount: i64) {
    if amount <= 0 || owner.is_none() {
        return;
    }
    shadow_flow(world, None, owner, amount, Flow::Tax, true);
    *world.probe.treasury_out.entry("Tax refund".to_string()).or_insert(0) += amount;
    world.purse_add(None, -amount);
    world.purse_add(owner, amount);
    world.stats.current.flow_tax -= amount;
}

/// The surplus credit (`[treasury] credit_max`, `economy::collect_wage` at
/// a negative tax rate): the Treasury pays `coins` to `agent` on a wage,
/// at most its coins above `band[0]`. Ledgered as `jobs.tax_credit`, not
/// in `flow_tax`: the civic budget's receipts stay the gross tax. Conserves
/// coins. Returns the coins paid.
pub fn tax_credit(world: &mut World, agent: EntityId, coins: i64) -> i64 {
    let surplus = world.purse(None) - world.config.treasury.band[0];
    let coins = coins.min(surplus);
    if coins <= 0 {
        return 0;
    }
    shadow_flow(world, None, Some(agent), coins, Flow::Tax, false);
    *world.probe.treasury_out.entry("Tax credit".to_string()).or_insert(0) += coins;
    world.purse_add(None, -coins);
    world.purse_add(Some(agent), coins);
    world.stats.current.jobs.tax_credit += coins;
    coins
}

/// L2 L11 (M17 plan O6): coins cross into the city from an outside
/// account: `from`'s treasury pays (never below 0), `to`'s purse gains,
/// the ledger column moves, `outside.inbound` grows. Untaxed (an export's
/// tax is the seller's explicit `pay`). Returns the coins moved.
pub fn cross_in(
    world: &mut World,
    from: crate::outside::OutsideId,
    to: Option<EntityId>,
    amount: i64,
    flow: Flow,
) -> i64 {
    if amount <= 0 {
        return 0;
    }
    let Some(f) = world.outside.faction_mut(from) else { return 0 };
    let moved = amount.min(f.treasury.max(0));
    if moved <= 0 {
        return 0;
    }
    f.treasury -= moved;
    f.income_today -= moved;
    // M17 phase 4: a parent's crossings, for the balance of trade.
    if f.parent.is_some() {
        world.outside.today.parents_in += moved;
    }
    world.outside.inbound += moved;
    ledger(world, flow, moved);
    if matches!(owner_kind(world, to), OwnerKind::City) {
        *world.probe.treasury_in.entry(format!("{flow:?} (from outside)")).or_insert(0) += moved;
    }
    world.purse_add(to, moved);
    if to.is_some_and(|g| world.config.assets.enabled && world.has::<Gang>(g)) {
        world.stats.current.gang_income += moved;
    }
    moved
}

/// Real economy (plan E5): coins cross out of the city to an outside
/// account: `from`'s purse pays (`amount` in full for a corp or the city
/// with `full`, else capped at the purse), the account's treasury and
/// `income_today` gain, `outside.outbound` grows, the ledger column moves
/// (`ledger_out`). Untaxed. Returns the coins moved.
pub fn cross_out(
    world: &mut World,
    from: Option<EntityId>,
    to: crate::outside::OutsideId,
    amount: i64,
    flow: Flow,
    full: bool,
) -> i64 {
    if amount <= 0 || world.outside.faction(to).is_none() {
        return 0;
    }
    let moved = if full { amount } else { amount.min(world.purse(from).max(0)) };
    if moved <= 0 {
        return 0;
    }
    ledger_out(world, flow, moved);
    shadow_flow(world, from, None, moved, flow, false);
    if matches!(owner_kind(world, from), OwnerKind::City) {
        *world.probe.treasury_out.entry(format!("{flow:?} (to outside)")).or_insert(0) += moved;
    }
    world.purse_add(from, -moved);
    if let Some(f) = world.outside.faction_mut(to) {
        f.treasury += moved;
        f.income_today += moved;
        if f.parent.is_some() {
            world.outside.today.parents_out += moved;
        }
    }
    world.outside.outbound += moved;
    moved
}

/// Plan E5: why a purse imports (the `Imported` event names the Food case).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ImportWhy {
    /// `assets::buy`'s D30 import of a new asset, a robot included.
    Asset,
    /// `assets::repairs`' missing Parts.
    Repair,
    /// A gang's cook or a Market's legal Stims restock.
    Stims,
    /// A self-installed ICE tier.
    Ice,
    /// E11: a Market's food from the World at the ask.
    Food,
    /// Hunger as sickness P5h: a Home or a floor the Treasury builds toward jobs.
    Build,
}

/// Plan E5: an import, paid by `from`. With the market off it is
/// `charge(from, None, amount, Flow::Import)` exactly (the City's customs,
/// M13 D8); with it on the coins cross out to the World (`Flow::Import`,
/// `flow_import_out`) and customs `round(customs_rate × moved)` go to the
/// Treasury on top (`Flow::Customs`). Returns the coins that left for the good.
pub fn import(world: &mut World, from: Option<EntityId>, amount: i64, _why: ImportWhy) -> i64 {
    let full = matches!(owner_kind(world, from), OwnerKind::City | OwnerKind::Corp(_));
    let moved = cross_out(world, from, crate::outside::WORLD_ACCOUNT, amount, Flow::Import, full);
    if moved > 0 {
        let customs = (crate::systems::world_market::customs_rate(world) * moved as f32).round() as i64;
        if customs > 0 {
            charge(world, from, None, customs, Flow::Customs);
        }
        // Jobs & lives phase 1 (§ 2): the crossing's handling at a Yard.
        crate::systems::industry::handle_import(world, from, moved);
    }
    moved
}

/// M16a (plan C5): coins from `from`'s purse into a contract record's
/// escrow (a game abstraction: a balance held on the record), capped as
/// `pay` (`min(amount, max(purse, 0))`). Ledger `Flow::Escrow` (capital: a
/// corp's posting is not a week of losses). Returns the coins moved.
pub fn escrow_in(world: &mut World, from: Option<EntityId>, id: crate::contract::ContractId, amount: i64) -> i64 {
    if amount <= 0 || !world.contracts.contains_key(&id) {
        return 0;
    }
    let moved = amount.min(world.purse(from).max(0));
    if moved <= 0 {
        return 0;
    }
    ledger(world, Flow::Escrow, moved);
    shadow_flow(world, from, None, moved, Flow::Escrow, false);
    world.purse_add(from, -moved);
    uncount_cashflow(world, from, moved);
    if let Some(c) = world.contracts.get_mut(&id) {
        c.escrow += moved;
    }
    world.escrow_held += moved;
    moved
}

/// M16a (plan C5): coins out of a record's escrow to `to` (`None` the
/// Treasury), at most what it holds: tax withheld when `flow` is taxed,
/// ledger `flow` (unless a refund, `Flow::Escrow`) and `Flow::Escrow`
/// negative; a gang payee's take is gang income. Returns the coins moved.
pub fn escrow_out(
    world: &mut World,
    id: crate::contract::ContractId,
    to: Option<EntityId>,
    amount: i64,
    flow: Flow,
) -> i64 {
    let held = world.contracts.get(&id).map_or(0, |c| c.escrow);
    let moved = amount.min(held);
    if moved <= 0 {
        return 0;
    }
    if let Some(c) = world.contracts.get_mut(&id) {
        c.escrow -= moved;
    }
    world.escrow_held -= moved;
    ledger(world, Flow::Escrow, -moved);
    if flow != Flow::Escrow {
        ledger(world, flow, moved);
    }
    shadow_flow(world, None, to, moved, flow, flow == Flow::Escrow);
    let tax = match to {
        Some(payee) if flow.taxed() => withhold(world, payee, moved),
        _ => 0,
    };
    world.purse_add(to, moved - tax);
    if tax > 0 {
        world.purse_add(None, tax);
        world.stats.current.flow_tax += tax;
    }
    if to.is_some_and(|g| world.config.assets.enabled && world.has::<Gang>(g)) {
        world.stats.current.gang_income += moved - tax;
    }
    if flow.capital() {
        uncount_cashflow(world, to, -(moved - tax));
    }
    if crate::systems::wages::on(world) {
        note_revenue(world, to, flow, moved - tax);
        crate::systems::wages::note_inflow(world, to, flow, moved - tax);
    }
    moved
}

/// Real economy E28 (the escrow precedent): coins from `from`'s purse
/// (`None` the Treasury) into a Mission's `Charity.purse`, capped as `pay`
/// for an agent or a gang, the full amount for a corp or the city. Ledger
/// `Flow::Donate` (untaxed). Returns the coins moved.
pub fn charity_in(world: &mut World, from: Option<EntityId>, mission: EntityId, amount: i64) -> i64 {
    if amount <= 0 || world.comp::<Building>(mission).is_none_or(|b| b.charity.is_none()) {
        return 0;
    }
    let full = matches!(owner_kind(world, from), OwnerKind::City | OwnerKind::Corp(_));
    let moved = if full { amount } else { amount.min(world.purse(from).max(0)) };
    if moved <= 0 {
        return 0;
    }
    ledger(world, Flow::Donate, moved);
    shadow_flow(world, from, Some(mission), moved, Flow::Donate, false);
    world.purse_add(from, -moved);
    if let Some(c) = world.comp_mut::<Building>(mission).and_then(|b| b.charity.as_mut()) {
        c.purse += moved;
    }
    moved
}

/// Jobs & lives phase 6 (§ 8): a follower's tithe into a church's purse
/// (`Flow::Tithe`, untaxed; at most the follower's coins). Returns the
/// coins moved.
pub fn tithe_in(world: &mut World, from: EntityId, mission: EntityId, amount: i64) -> i64 {
    if amount <= 0 || world.comp::<Building>(mission).is_none_or(|b| b.charity.is_none()) {
        return 0;
    }
    let moved = amount.min(world.purse(Some(from)).max(0));
    if moved <= 0 {
        return 0;
    }
    ledger(world, Flow::Tithe, moved);
    shadow_flow(world, Some(from), Some(mission), moved, Flow::Tithe, false);
    world.purse_add(Some(from), -moved);
    if let Some(c) = world.comp_mut::<Building>(mission).and_then(|b| b.charity.as_mut()) {
        c.purse += moved;
    }
    moved
}

/// Real economy E27 (the escrow precedent): coins out of a Mission's purse
/// to `to` (`None` the Treasury), at most what it holds: tax withheld when
/// `flow` is taxed, the ledger line, a gang payee's take as gang income.
/// Returns the coins moved.
pub fn charity_out(world: &mut World, mission: EntityId, to: Option<EntityId>, amount: i64, flow: Flow) -> i64 {
    let held = world.comp::<Building>(mission).and_then(|b| b.charity.as_ref()).map_or(0, |c| c.purse);
    let moved = amount.min(held);
    if moved <= 0 {
        return 0;
    }
    if let Some(c) = world.comp_mut::<Building>(mission).and_then(|b| b.charity.as_mut()) {
        c.purse -= moved;
    }
    ledger(world, flow, moved);
    shadow_flow(world, Some(mission), to, moved, flow, false);
    let tax = match to {
        Some(payee) if flow.taxed() => withhold(world, payee, moved),
        _ => 0,
    };
    world.purse_add(to, moved - tax);
    if tax > 0 {
        world.purse_add(None, tax);
        world.stats.current.flow_tax += tax;
    }
    if to.is_some_and(|g| world.config.assets.enabled && world.has::<Gang>(g)) {
        world.stats.current.gang_income += moved - tax;
    }
    moved
}

/// Real economy E33 (phase 3a): pay `amount` (capped at the till) from the
/// Recycler's till to `to`, untaxed, with the ledger line (the escrow and
/// charity precedent). Returns the coins moved (an empty till pays 0).
pub fn till_out(world: &mut World, to: Option<EntityId>, amount: i64, flow: Flow) -> i64 {
    let moved = amount.min(world.econ.recycler_till);
    if moved <= 0 {
        return 0;
    }
    world.econ.recycler_till -= moved;
    ledger(world, flow, moved);
    shadow_flow(world, None, to, moved, flow, false);
    world.purse_add(to, moved);
    if crate::systems::wages::on(world) {
        crate::systems::wages::note_inflow(world, to, flow, moved);
    }
    moved
}

/// Gross coins earned through a building today.
pub fn credit(world: &mut World, building: EntityId, coins: i64) {
    if let Some(b) = world.comp_mut::<Building>(building) {
        b.revenue_today += coins;
    }
}
