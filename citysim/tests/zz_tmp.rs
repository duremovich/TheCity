use citysim::systems::{demography, street};
use citysim::{Brain, BuildingKind, Config, Household, Lod, Squatter, Wallet, World, TICKS_PER_DAY};

#[test]
fn zz_where_homeless() {
    let mut w = World::new(42, Config::load());
    for day in (0..70).step_by(1) {
        w.run_ticks(TICKS_PER_DAY);
        if day < 28 || day % 6 != 0 {
            continue;
        }
        let (mut hl, mut hotel, mut squat, mut street_n, mut stat, mut coins_ge3, mut coins0) = (0, 0, 0, 0, 0, 0, 0);
        for a in w.citizens() {
            if !w.has::<Brain>(a) || !demography::is_adult(&w, a) {
                continue;
            }
            if !matches!(w.comp::<Household>(a), Some(Household { home: None, .. })) {
                continue;
            }
            hl += 1;
            if w.comp::<Brain>(a).is_some_and(|b| b.lod == Lod::Statistical) {
                stat += 1;
            }
            let c = w.comp::<Wallet>(a).map_or(0, |x| x.coins);
            if c >= 3 {
                coins_ge3 += 1;
            }
            if c == 0 {
                coins0 += 1;
            }
            if street::booked_hotel(&w, a).is_some() {
                hotel += 1;
            } else if w.has::<Squatter>(a) {
                squat += 1;
            } else {
                street_n += 1;
            }
        }
        let hotels = w.buildings_of_kind(BuildingKind::Hotel).len();
        let beds: u32 = w
            .buildings_of_kind(BuildingKind::Hotel)
            .iter()
            .filter_map(|&h| w.comp::<citysim::Building>(h))
            .map(|b| u32::from(b.capacity))
            .sum();
        let derelicts = w
            .with::<citysim::Building>()
            .into_iter()
            .filter(|&b| w.comp::<citysim::Building>(b).is_some_and(|x| x.derelict))
            .count();
        eprintln!(
            "d{day}: homeless {hl} (stat {stat}): hotel {hotel} squat {squat} street {street_n}; coins>=3 {coins_ge3} coins0 {coins0}; hotels {hotels} beds {beds}; derelicts {derelicts}; night_price {}",
            w.config.street.night_price
        );
    }
}
