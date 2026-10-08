//! L2 (plan L23-L25, "Ledger maths"): faction violence's ledger. Phase 3
//! builds the actor side: the on-screen members' acts (`note_act`) and
//! member-days (`daily_actor`) per `(Order(o), district, Member)` cell, and
//! the Statistical GangWork day that reads them as per-member-day rates
//! (`stat_gang_day`). Phase 4 adds the victim side.
//!
//! What this is: counters and one seeded dice roll per Statistical member
//! and day on a keyed stream (`WordNs::GangHour`); "extortion", "claim" and
//! "deal" are a gang's game actions on buildings and purses of fictional
//! agents of a simulated city.

use rand::Rng;

use crate::components::{Brain, DistrictId, Household, Lod, Order, Sentence};
use crate::entity::EntityId;
use crate::ledger::{ActKind, VictimClass, ViolenceSource, RATE_DAYS};
use crate::world::World;

/// The district a member's acts and member-days are filed under: its
/// Home's, else its gang's Hideout's.
pub fn member_district(world: &World, id: EntityId) -> Option<DistrictId> {
    if let Some(h) = world.comp::<Household>(id).and_then(|h| h.home) {
        return Some(world.district_of_building(h));
    }
    world.gang_of(id).and_then(|g| world.hideout_of(g)).map(|h| world.district_of_building(h))
}

/// L25 (actor side): a body member following its gang's order did one of
/// the order's acts (an extortion, a claim blow, a deal shift): counted in
/// `(Order(o), its district, Member)`. Statistical acts are not counted
/// (the ledger is what bodies do).
pub fn note_act(world: &mut World, actor: EntityId, kind: ActKind) {
    if !crate::systems::lod::budget_on(world) {
        return;
    }
    if world.comp::<Brain>(actor).is_none_or(|b| b.lod == Lod::Statistical) {
        return;
    }
    let Some(o) = crate::systems::gang::following_order(world, actor) else { return };
    let Some(d) = member_district(world, actor) else { return };
    let cell = world.order_rates.cell_mut((ViolenceSource::Order(o), d, VictimClass::Member));
    let a = &mut cell.acts_today[kind as usize];
    *a = a.saturating_add(1);
}

/// Midnight, before the hour's LOD assignment: one member-day for every
/// free member that held a body at any assignment yesterday and follows
/// its gang's order, then every cell's windows roll and empty cells go.
pub fn daily_actor(world: &mut World) {
    let yesterday = world.day().saturating_sub(1);
    let mut days: Vec<(ViolenceSource, DistrictId)> = Vec::new();
    for gang in world.gangs() {
        let members = world.comp::<crate::components::Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
        for m in members {
            if world.has::<Sentence>(m) || world.comp::<Brain>(m).is_none_or(|b| b.body_day != Some(yesterday)) {
                continue;
            }
            let Some(o) = crate::systems::gang::following_order(world, m) else { continue };
            let Some(d) = member_district(world, m) else { continue };
            days.push((ViolenceSource::Order(o), d));
        }
    }
    for (s, d) in days {
        let cell = world.order_rates.cell_mut((s, d, VictimClass::Member));
        cell.member_today = cell.member_today.saturating_add(1);
    }
    for cell in world.order_rates.cells.values_mut() {
        cell.roll(RATE_DAYS);
    }
    world.order_rates.cells.retain(|_, c| !c.is_empty());
}

/// The prior of an act per member-day (`[gangs] stat_*_prior`).
fn prior(world: &World, kind: ActKind) -> f32 {
    let g = &world.config.gangs;
    match kind {
        ActKind::Extort => g.stat_extort_prior,
        ActKind::Claim => g.stat_claim_prior,
        ActKind::Deal => g.stat_deal_prior,
    }
}

/// Ledger maths, actor side: `p = (Σ acts + prior × prior_days) ÷
/// (Σ member_days + prior_days)` per member-day over the cell's window.
pub fn act_rate(world: &World, o: Order, d: DistrictId, kind: ActKind) -> f32 {
    let days = world.config.gangs.stat_prior_days.max(0.0);
    let p0 = prior(world, kind);
    let (acts, members) = world
        .order_rates
        .cells
        .get(&(ViolenceSource::Order(o), d, VictimClass::Member))
        .map_or((0, 0), |c| (c.acts_sum(kind), c.member_days_sum()));
    let denom = members as f32 + days;
    if denom <= 0.0 {
        return p0;
    }
    ((acts as f32 + p0 * days) / denom).clamp(0.0, 1.0)
}

/// The act a Statistical member's GangWork day would be: a deal shift for
/// a dealer whose gang holds a batch, a claim blow under Squat, an
/// extortion under Expand, Contest and VirtRaid; none otherwise.
fn day_act(world: &World, id: EntityId, o: Order) -> Option<ActKind> {
    let stims = crate::systems::stims::on(world);
    if stims && crate::systems::stims::is_dealer(world, id) {
        let stock = world
            .gang_of(id)
            .and_then(|g| world.hideout_of(g))
            .map_or(0, |h| world.stock(h, crate::components::Good::Stims));
        if stock >= world.config.stims.deal_batch {
            return Some(ActKind::Deal);
        }
    }
    match o {
        Order::Squat => Some(ActKind::Claim),
        Order::Expand | Order::Contest | Order::VirtRaid => Some(ActKind::Extort),
        _ => None,
    }
}

/// L23: the Statistical GangWork day, from `lod::stat_work`'s jobless
/// branch in the Work phase. A free Statistical member following its
/// gang's order, its task not done today: one draw on `WordNs::GangHour`
/// keyed `(day, id.index)` (the same draw every hour of the day, so one
/// trial a day) against its cell's rate; a hit acts at once (no walk, no
/// witness: a Statistical agent has no eyes) and, on the same stream at
/// the table's `p_theft_caught`, an extortion or a deal is reported and the
/// member gets a body for the arrest path, as `stat_theft`. A hit with no
/// target today waits for the next hour.
pub fn stat_gang_day(world: &mut World, id: EntityId) {
    use crate::components::Crime;
    if !world.has::<crate::components::GangMember>(id) || world.has::<Sentence>(id) {
        return;
    }
    let today = world.day();
    if world.comp::<Brain>(id).is_none_or(|b| b.gang_task_day == Some(today) || b.lod != Lod::Statistical) {
        return;
    }
    let Some(gang) = world.gang_of(id) else { return };
    let Some(o) = crate::systems::gang::following_order(world, id) else { return };
    let Some(kind) = day_act(world, id, o) else { return };
    let Some(d) = member_district(world, id) else { return };
    let p = act_rate(world, o, d, kind);
    let mut rng = world.rng.word(crate::word::WordNs::GangHour, today, u64::from(id.index));
    let u: f32 = rng.random();
    if u >= p {
        return;
    }
    let caught_u: f64 = rng.random();
    let crime = match kind {
        ActKind::Deal => {
            let Some(bar) = crate::systems::stims::deal_bar(world, gang) else { return };
            // The shift's doses sell through the Statistical buyers' pass
            // (`stims::stat_addicts` reads `deal_log`).
            world.deal_log.insert(bar, (id, today));
            if let Some(b) = world.comp_mut::<Brain>(id) {
                b.gang_task_day = Some(today);
            }
            world.stats.current.budget.stat_deals += 1;
            Some(Crime::Dealing)
        }
        ActKind::Claim => {
            let Some((b, _)) = crate::systems::gang::gang_work_target(world, id) else { return };
            if !crate::systems::street::is_derelict(world, b) {
                return;
            }
            crate::systems::gang::squat_claim_with(world, id, b, false);
            world.stats.current.budget.stat_claims += 1;
            None
        }
        ActKind::Extort => {
            let Some(home) = crate::systems::gang::stat_extort_target(world, id, o) else { return };
            crate::systems::gang::stat_extort(world, id, home);
            world.stats.current.budget.stat_extorts += 1;
            Some(Crime::Extortion)
        }
    };
    let Some(crime) = crime else { return };
    let p_caught = world
        .stat_table
        .as_ref()
        .and_then(|t| t.p_theft_caught)
        .map_or(world.config.crime.stat_theft_caught_p, f64::from);
    if caught_u < p_caught {
        crate::systems::law::file_report(world, crime, id, None);
        crate::systems::lod::set_lod(world, id, Lod::Coarse);
    }
}
