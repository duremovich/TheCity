use super::*;

// ---------------------------------------------------------------------------
// Venues: the door, the price, the visit (plan L19)
// ---------------------------------------------------------------------------

pub(super) fn night(tod: u16) -> bool {
    !(360..1320).contains(&tod)
}

/// Plan L19: a venue is open: standing, not closed (a riot, a Crackdown,
/// `CloseLeisure`), not under a night curfew (22:00-06:00), and a Den only
/// while gambling is legal.
pub fn open(world: &World, b: EntityId) -> bool {
    let Some(bd) = world.comp::<Building>(b) else { return false };
    if bd.demolished || bd.derelict || world.is_closed(b) {
        return false;
    }
    let d = world.district_of_building(b);
    if world.levers.curfew.get(d.index()).copied().unwrap_or(false) && night(world.tick_of_day()) {
        return false;
    }
    !(bd.kind == BuildingKind::Den && world.levers.ban_gambling)
}

/// Room for `id`: inside already, or fewer occupants than the capacity.
pub(super) fn has_room(world: &World, id: EntityId, b: EntityId) -> bool {
    // Plan L19, L30: the live Seat (and Bed) reservations of others count
    // (`World::reserved_at`: Scale S4 perf, the per-building index; it was
    // one pass over every reservation per ladder walk).
    world.comp::<Building>(b).is_some_and(|bd| {
        bd.occupants.contains(&id) || bd.occupants.len() + world.reserved_at(b, id) < usize::from(bd.capacity)
    })
}

/// Plan L19: the door policy. A Lounge refuses a Dreg or a dress below 2; a
/// Club refuses `rep.heat ≥ door_heat` when the strongest Host on shift has
/// courage above that heat (no draw).
pub fn door_ok(world: &World, id: EntityId, b: EntityId) -> bool {
    // Jobs & lives phase 1 (§ 2): a site still being built is shut, and a
    // dark district's night venues (all but the noodle stalls) are closed.
    if crate::systems::industry::closed_site(world, b) {
        return false;
    }
    if crate::systems::industry::dark_at(world, b)
        && kind_of(world, b).is_some_and(|k| k.is_leisure() && k != BuildingKind::NoodleBar)
    {
        return false;
    }
    // The pressure pass phase 2 (OPEN-176): a bouncer on duty turns a
    // visitor with a cause away (drunk, a brawler, a rival's colours, rags).
    if crate::systems::customers::door_cause(world, id, b).is_some() {
        return false;
    }
    // The pressure pass R6a (OPEN-217): a rough client barred by the madam.
    if crate::systems::vice::barred(world, id, b) {
        return false;
    }
    // Round 4 T4 (the under-18 rule): no vice house takes a client under 18,
    // a bouncer on the door or not (`ages::is_protected`).
    if kind_of(world, b).is_some_and(|k| crate::systems::vice::is_vice_kind(k) || k == BuildingKind::Dollhouse)
        && crate::systems::ages::is_protected(world, id)
    {
        return false;
    }
    match kind_of(world, b) {
        Some(BuildingKind::Lounge) => lounge_admits(world, id),
        // Jobs & lives phase 5: open while a contracted worker is on shift.
        Some(BuildingKind::Dollhouse) => crate::systems::dollhouse::staffed_now(world, b),
        Some(BuildingKind::Club) => {
            let heat = crate::systems::reputation::rep(world, id).heat;
            if heat < world.config.leisure.door_heat {
                return true;
            }
            let tod = world.tick_of_day();
            // Jobs and room J13 (`service`): the trade staff on duty at the
            // door (a fighting trade: the Bouncers) stand with the Hosts.
            let bouncer = ownership::staff_at(world, b)
                .into_iter()
                .filter(|&s| world.comp::<Job>(s).is_some_and(|j| j.role == Role::Host && j.on_shift(tod)))
                .chain(crate::systems::trades::door_staff(world, b))
                .max_by(|&x, &y| {
                    crate::systems::combat::power(world, x)
                        .total_cmp(&crate::systems::combat::power(world, y))
                        .then(y.cmp(&x))
                });
            !bouncer.is_some_and(|h| crate::systems::law::courage(world, h) > heat)
        }
        _ => true,
    }
}

/// Plan L19: a Lounge's door policy, the visitor's alone (no Dreg, a dress
/// of 2 or better): the same answer at every Lounge.
fn lounge_admits(world: &World, id: EntityId) -> bool {
    crate::systems::classes::class_of(world, id) != Class::Dreg
        && crate::systems::reputation::appearance_of(world, id).dress >= 2
}

/// [`door_seen`]: is a venue of `kind` a door the bouncer calls at the door
/// (`[shift.counter]` on and a kept kind), not one the patron reads?
/// The pressure pass phase 2: every door a bouncer keeps is his call at the
/// door (`[customers]`), not the Club's alone.
fn door_called_at(world: &World, kind: BuildingKind) -> bool {
    crate::systems::shift::counter_on(world)
        && (kind == BuildingKind::Club
            || (crate::systems::customers::cfg(world).is_some() && crate::systems::customers::kept_kind(world, kind)))
}

/// The view milestone V2 (OPEN-265): does [`door_seen`] turn `id` away from
/// every venue of `kind` on a rule of the visitor's alone (a Lounge's class
/// and dress)? The ladder's searches skip the kind then, the same pick (none)
/// without walking every venue of it in reach: the 100k Statistical evening
/// at 21:00 asked a Lounge's door 1.86 M times, 14 s a day.
pub(super) fn door_bars_kind(world: &World, id: EntityId, kind: BuildingKind) -> bool {
    kind == BuildingKind::Lounge && !door_called_at(world, kind) && !lounge_admits(world, id)
}

/// The pre-view pass phase 3 (OPEN-121): the door as a patron choosing a
/// venue knows it. With `[shift.counter]` on, a Club's door is the
/// bouncer's call made at the door (`door_ok` there, `service::refused`),
/// not one the patron reads from across the city: the pick passes the
/// Club's heat rule and only passes over a venue that turned `id` away
/// within `[exec] refused_ticks`. Off: `door_ok` (the bouncer's verdict
/// read at the pick, so the door never refused anyone).
pub fn door_seen(world: &World, id: EntityId, b: EntityId) -> bool {
    if !kind_of(world, b).is_some_and(|k| door_called_at(world, k)) {
        return door_ok(world, id, b);
    }
    let turned = world
        .comp::<Brain>(id)
        .and_then(|br| br.refused)
        .is_some_and(|(r, t)| r == b && world.tick.saturating_sub(t) < world.config.exec.refused_ticks.max(1));
    // Round 4 T7 (OPEN-233): a face the house barred (a rough night, or a
    // return to a door that turned him away) knows it and goes elsewhere;
    // the barred came back to be turned away "barred" every night.
    let barred = crate::systems::vice::barred(world, id, b);
    !turned
        && !barred
        && !crate::systems::industry::closed_site(world, b)
        && !crate::systems::industry::dark_at(world, b)
}

/// The venue's price today (`Venue.price`; a Bar's drink is 2).
pub fn price_of(world: &World, b: EntityId) -> i64 {
    let Some(bd) = world.comp::<Building>(b) else { return 0 };
    if bd.kind == BuildingKind::Bar {
        return 2;
    }
    bd.venue.as_ref().map_or(0, |v| v.price)
}

/// L2 L14: inside a Club or Den with leisure on (a Drink there).
pub fn drink_venue(world: &World, id: EntityId) -> bool {
    here(world, id).and_then(|b| kind_of(world, b)).is_some_and(|k| matches!(k, BuildingKind::Club | BuildingKind::Den))
}

/// A Drink at a Club or Den: the visit is the venue's (a Bar's is not),
/// unless the drink is part of an Unwind whose main step counts it.
pub fn note_drink(world: &mut World, id: EntityId) {
    if !drink_venue(world, id) {
        return;
    }
    add_fun(world, id, world.config.leisure.gain.drink);
    let unwinding = world.comp::<Brain>(id).and_then(|b| b.plan_goal()) == Some(GoalKind::Unwind);
    if !unwinding {
        if let Some(b) = here(world, id) {
            crate::systems::jobs::note_visit(world, b);
        }
    }
}

/// A Bar drink also lifts `fun` (the cheap rung) with leisure on.
pub fn drink_fun(world: &mut World, id: EntityId) {
    if here(world, id).and_then(|b| kind_of(world, b)) == Some(BuildingKind::Bar) {
        add_fun(world, id, world.config.leisure.gain.drink);
    }
}

/// The fun an `Enjoy` (or a meal, a table) at `b` gives.
pub(crate) fn gain_at(world: &World, b: EntityId) -> f32 {
    let g = &world.config.leisure.gain;
    match world.comp::<Building>(b).map(|bd| (bd.kind, bd.tier)) {
        Some((BuildingKind::Club, 2)) => g.club_spire,
        Some((BuildingKind::Club, _)) => g.club,
        Some((BuildingKind::Arcade, _)) => g.arcade,
        Some((BuildingKind::NoodleBar, _)) => g.noodle_bar,
        Some((BuildingKind::FightPit, _)) => g.fight_pit,
        Some((BuildingKind::Den, _)) => g.den,
        Some((BuildingKind::Lounge, _)) => g.lounge,
        Some((BuildingKind::Bar, _)) => g.drink,
        // Jobs & lives phase 4: the Vice venues and the Boutique.
        Some((k @ (BuildingKind::StripClub | BuildingKind::Brothel), _)) => crate::systems::vice::gain(world, k),
        Some((BuildingKind::Boutique, _)) => world.config.fashion.gain_boutique,
        // Jobs & lives phase 5.
        Some((BuildingKind::Dollhouse, _)) => world.config.vice.gain_dollhouse,
        _ => 0.0,
    }
}

/// Belonging an `Enjoy` gives (spec: Club +0.2, Lounge +0.3).
pub(super) fn belonging_at(world: &World, b: EntityId) -> f32 {
    match kind_of(world, b) {
        Some(BuildingKind::Club) => 0.2,
        Some(BuildingKind::Lounge) => 0.3,
        _ => 0.0,
    }
}

/// A payment into a venue (`Flow::Leisure` or `Flow::Gamble`): `pay` and
/// `credit`, the `SetLeisureTax` surcharge (plan L37), the front's take.
pub(crate) fn pay_venue(world: &mut World, from: EntityId, b: EntityId, amount: i64, flow: Flow) -> i64 {
    let owner = world.owner_of(b);
    let paid = ownership::pay(world, Some(from), owner, amount, flow);
    if paid <= 0 {
        return 0;
    }
    ownership::credit(world, b, paid);
    note_paid(world, from, b, paid);
    let tax = world.levers.leisure_tax;
    if tax > 0.0 && owner.is_some() {
        let extra = (tax * paid as f32).round() as i64;
        ownership::pay(world, owner, None, extra, Flow::Tax);
    }
    book_take(world, b, paid, flow == Flow::Gamble);
    paid
}

/// A paid visit of `paid` coins by `payer` at `b`, by its kind
/// (`CoinProbe::venue_paid`; a body's in `venue_unmirrored` too).
pub(crate) fn note_paid(world: &mut World, payer: EntityId, b: EntityId, paid: i64) {
    if paid <= 0 {
        return;
    }
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return };
    let e = world.probe.venue_paid.entry(format!("{kind:?}")).or_default();
    e.0 += 1;
    e.1 += paid;
    note_unmirrored(world, payer, kind, 1);
}

/// OPEN-106: a body's visit (`lod::kept_body`) at a venue of `kind`, `+1`
/// or `-1` on a refund (`CoinProbe::venue_unmirrored`).
pub(super) fn note_unmirrored(world: &mut World, payer: EntityId, kind: BuildingKind, delta: i64) {
    if crate::systems::lod::kept_body(world, payer) {
        let e = world.probe.venue_unmirrored.entry(format!("{kind:?}")).or_default();
        *e = e.saturating_add_signed(delta);
    }
}

/// OPEN-106: a world-side pass's paid visit by `payer` at `b`, both tiers
/// alike (`classpay::status_daily`): out of `calibrate`'s `p_venue` (a
/// body's is already counted out by `note_paid`).
pub(crate) fn note_world_side(world: &mut World, payer: EntityId, b: EntityId) {
    if crate::systems::lod::kept_body(world, payer) {
        return;
    }
    let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) else { return };
    *world.probe.venue_unmirrored.entry(format!("{kind:?}")).or_default() += 1;
}

/// Undo `pay_venue`'s `SetLeisureTax` surcharge on `paid` (plan L37): the
/// Treasury hands `owner` back the surcharge (`ownership::refund_tax`).
pub(super) fn refund_surcharge(world: &mut World, owner: Option<EntityId>, paid: i64) {
    let tax = world.levers.leisure_tax;
    if tax > 0.0 && owner.is_some() && paid > 0 {
        let extra = (tax * paid as f32).round() as i64;
        ownership::refund_tax(world, owner, extra);
    }
}

/// The house's take: `take_today` (a table's margin) and, on a front its
/// gang owns, `take_week` (review fix: a seeded front on the city's deed
/// books no week, its coins reached the Treasury).
pub(super) fn book_take(world: &mut World, b: EntityId, delta: i64, table: bool) {
    let Some(bd) = world.comp_mut::<Building>(b) else { return };
    let gang_owned = bd.venue.as_ref().and_then(|v| v.front_of).is_some_and(|g| bd.owner == Some(g));
    if let Some(v) = bd.venue.as_mut() {
        if table {
            v.take_today += delta;
        }
        if gang_owned {
            v.take_week += delta;
        }
    }
}

/// The house pays a win (`Flow::GambleWin`, untaxed): a corp or the city in
/// full (`charge`), an agent or a gang what it holds (`pay`). A house short
/// of the win shuts its table for the day.
pub(super) fn house_pays(world: &mut World, b: EntityId, to: EntityId, amount: i64, gross: i64) -> i64 {
    let owner = world.owner_of(b);
    let paid = ownership::charge(world, owner, Some(to), amount, Flow::GambleWin);
    let today = world.day();
    book_take(world, b, -paid, true);
    if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
        if paid < amount {
            v.table_shut_day = Some(today);
        }
    }
    ownership::credit(world, b, -paid);
    // Deviation: a win is big at `big_win` or more of the gross return (the
    // stake back and the win): with the spec's `stake_cap` 20 nothing ever
    // paid more than 40, so `> big_win` (40) could never fire.
    let gross = gross - (amount - paid);
    if gross >= world.config.leisure.big_win {
        let text = format!("{} won {gross} at {}", world.name_of(to), world.name_of(b));
        world.push_event(EventKind::Gambled, &[to, b], text);
    }
    paid
}

/// Spec § 1: a stake is `min(stake_frac × coins, stake_cap)`, at least 1
/// and at most what the purse holds.
pub fn stake_of(world: &World, coins: i64) -> i64 {
    let cfg = &world.config.leisure;
    ((cfg.stake_frac * coins as f32).floor() as i64).min(cfg.stake_cap).max(1).min(coins.max(0))
}

pub(super) fn table_open(world: &World, b: EntityId) -> bool {
    let today = world.day();
    world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).is_none_or(|v| v.table_shut_day != Some(today))
}

/// The pressure pass R6a (OPEN-217): an off-screen visitor of the pit `b`
/// in the bet window with `gamble_rung_courage` and a coin stakes on
/// tonight's bout as a body's `Gamble` does (`Flow::Gamble` into the
/// house, the bet settled at `bout_hour`), at most `pit_bettors_max` a
/// pit. Returns the stake placed (0 none). Nothing with `[trade_verbs]` absent.
pub(super) fn bout_bet(world: &mut World, id: EntityId, b: EntityId) -> i64 {
    let Some(max) = world.config.trade_verbs.as_ref().map(|c| c.pit_bettors_max) else { return 0 };
    let brave = world.comp::<Personality>(id).is_some_and(|p| p.courage >= world.config.leisure.gamble_rung_courage);
    if !bet_window(world) || !brave {
        return 0;
    }
    let placed = world.comp::<Building>(b).and_then(|bd| bd.venue.as_ref()).map_or(usize::MAX, |v| v.bets.len());
    let left = spendable(world, id).min(coins(world, id));
    if placed >= max as usize || left < 1 {
        return 0;
    }
    let stake = stake_of(world, left);
    let paid = pay_venue(world, id, b, stake, Flow::Gamble);
    if paid <= 0 {
        return 0;
    }
    if let Some(v) = world.comp_mut::<Building>(b).and_then(|bd| bd.venue.as_mut()) {
        v.bets.push((id, paid, false));
    }
    paid
}

/// R6a: is the bet window open at the time of day `tod` (a body's arrival)?
pub(super) fn bet_window_at(world: &World, tod: u64) -> bool {
    let cfg = &world.config.leisure;
    let h = ((tod % TICKS_PER_DAY) / TICKS_PER_HOUR) as u16;
    h < cfg.bout_hour && cfg.bout_hour.saturating_sub(cfg.bet_window_hours) <= h && !world.levers.ban_gambling
}

/// A FightPit takes bets in the `bet_window_hours` before `bout_hour`.
pub(super) fn bet_window(world: &World) -> bool {
    let cfg = &world.config.leisure;
    let h = world.tick_of_day() / TICKS_PER_HOUR as u16;
    // Review fix: saturating, so a `bout_hour` below the window cannot wrap.
    h < cfg.bout_hour && cfg.bout_hour.saturating_sub(cfg.bet_window_hours) <= h && !world.levers.ban_gambling
}

/// Plan L18: one table game at the house `b`: the draw on `WordNs::Gamble`
/// keyed `(tick, id)` (or `stream` when given), a loss to the house, a win
/// paid by it. Returns the agent's net coins.
pub(super) fn play_table(world: &mut World, id: EntityId, b: EntityId, stake: i64, u: f32) -> i64 {
    let p_win = 0.5 - world.config.leisure.house_edge;
    if u < p_win {
        // Phase 7b's probe: a won round is a visit too (no coins in).
        if let Some(kind) = world.comp::<Building>(b).map(|bd| bd.kind) {
            world.probe.venue_paid.entry(format!("{kind:?}")).or_default().0 += 1;
            note_unmirrored(world, id, kind, 1);
        }
        house_pays(world, b, id, stake, 2 * stake)
    } else {
        -pay_venue(world, id, b, stake, Flow::Gamble)
    }
}
