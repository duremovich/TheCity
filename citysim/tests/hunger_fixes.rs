//! Hunger as sickness P1 (docs/HUNGER_SICKNESS.md § 6): the hunger fixes'
//! city-level behaviour (the person probes in `person.rs` are the single
//! agents).

use citysim::systems::life;
use citysim::{Config, EventKind, Lod, Wallet, World, TICKS_PER_DAY};

/// G3 (`hf_buy_not_steal`), coordinator call: in an all-Full city with no
/// dole (so some are broke), food thieves are the ones who cannot buy a
/// meal; nobody holding a meal's coins steals food, and the broke still do.
#[test]
fn test_g3_only_the_broke_steal_food() {
    let mut cfg = Config::load().dole_city().v1_profile();
    cfg.lod.force = Some(Lod::Full);
    let mut w = World::new(408, cfg);
    w.levers.dole_per_day = 0;
    let mut cursor = w.events.back().map_or(0, |e| e.id + 1);
    let (mut broke, mut able) = (0u32, Vec::new());
    let end = w.tick + 20 * TICKS_PER_DAY;
    while w.tick < end {
        w.run_ticks(1);
        let fresh: Vec<(citysim::EntityId, String)> = w
            .events
            .iter()
            .filter(|e| e.id >= cursor && e.kind == EventKind::Theft && e.text.contains("food"))
            .filter_map(|e| e.actors.first().map(|&a| (a, e.text.clone())))
            .collect();
        cursor = w.events.back().map_or(cursor, |e| e.id + 1);
        for (a, text) in fresh {
            let coins = w.comp::<Wallet>(a).map_or(0, |x| x.coins);
            if coins >= life::meal_price(&w, a) {
                able.push((a.index, coins, text));
            } else {
                broke += 1;
            }
        }
    }
    eprintln!("food thefts by the broke {broke}, by agents holding a meal's coins {}: {able:?}", able.len());
    assert!(broke > 0, "nobody broke stole food in 20 days without a dole");
    assert!(able.is_empty(), "thieves holding a meal's coins: {able:?}");
}
