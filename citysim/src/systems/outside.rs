//! The World account (`outside::WORLD_ACCOUNT`): created on first use and
//! booked by the World market (`world_market`). L2's flat-price export hook
//! that used to live here retired with the off switches (2026-10-09): the
//! market is the one World path.

use crate::outside::{OutsideFaction, OutsideKind, WORLD_ACCOUNT};
use crate::world::World;

/// The World account, created on first use (Real economy plan E4: the
/// market seeds it with `[world_market] treasury_ref`).
pub(crate) fn ensure_world_account(world: &mut World, treasury_ref: i64) {
    if world.outside.faction(WORLD_ACCOUNT).is_some() {
        return;
    }
    world.outside.factions.push(OutsideFaction {
        id: WORLD_ACCOUNT,
        name: "the World".to_string(),
        kind: OutsideKind::World,
        treasury: 0,
        treasury_ref,
        base_income: 0,
        base_upkeep: 0,
        income_today: 0,
        market: 1.0,
        dead: false,
        books: Default::default(),
    });
    world.outside.next_id = world.outside.next_id.max(WORLD_ACCOUNT + 1);
}
