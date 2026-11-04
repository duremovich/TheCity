use super::*;

// ---------------------------------------------------------------------------
// The nightly bout (plan L18)
// ---------------------------------------------------------------------------

/// Plan L18 (deviation: the bout is resolved here with `resolve_fight`'s
/// `p_win` and no death, its draw on `WordNs::Bout`; `resolve_fight` rolls
/// on the world stream, L31). At `bout_hour` each open FightPit pairs the
/// two Fighters on shift longest since their last bout (Shadow V4 bug 17:
/// the two fittest fought six nights running while the rest sat; ties the
/// fitter, then the lower id): the loser loses 0.3
/// energy and safety and remembers `Lost`; tonight's bets settle (a bettor
/// on the winner's side is paid twice the stake by the house); the sides
/// are drawn on the same stream per bettor by courage. A pit without two
/// Fighters refunds its bets.
pub fn bouts(world: &mut World) {
    let day = world.day();
    let tod = world.tick_of_day();
    for pit in world.buildings_of_kind(BuildingKind::FightPit).to_vec() {
        // Review fix: a pit that fell refunds its stranded bets too.
        let bets =
            world.comp_mut::<Building>(pit).and_then(|bd| bd.venue.as_mut()).map(|v| std::mem::take(&mut v.bets));
        let bets = bets.unwrap_or_default();
        // L2 shadow fixes item 9: a working day's shift, and in the pit (a
        // body inside, or a Statistical one credited by the clock): a
        // fighter won a bout while wandering Mid East on her rest day.
        let present = |f: EntityId| {
            world.comp::<Job>(f).is_some_and(|j| crate::exec::routine::is_workday(j.shift_key_at(world.tick)))
                && world.comp::<Brain>(f).is_some_and(|b| {
                    b.lod == Lod::Statistical || world.comp::<Position>(f).is_some_and(|p| p.building == Some(pit))
                })
        };
        // Round 6 V6 (OPEN-217): the pit's own book of who fought when (the
        // `Bout` memory, read before, is one an off-screen fighter's eight
        // slots can drop), else the memory (a save from before the book).
        let last_bout = |f: EntityId| {
            let book = crate::systems::vice::last_bout(world, f);
            if book > 0 {
                return book;
            }
            world
                .comp::<crate::components::Memory>(f)
                .and_then(|m| m.entries.iter().filter(|e| e.kind == MemoryKind::Bout).map(|e| e.tick).max())
                .unwrap_or(0)
        };
        // The under-18 rule (CLAUDE.md): nobody under 18 is on the card
        // (`ages::is_protected`; the Fighter's hire is 18 and up, this is the
        // explicit gate).
        let mut fighters: Vec<(f32, EntityId, u64)> = ownership::staff_at(world, pit)
            .into_iter()
            .filter(|&f| world.comp::<Job>(f).is_some_and(|j| j.role == Role::Fighter && j.on_shift(tod)))
            .filter(|&f| crate::systems::law::living(world, f) && !world.has::<Sentence>(f))
            .filter(|&f| !crate::systems::ages::is_protected(world, f))
            .filter(|&f| present(f))
            // The combat model C2: a fighter turned away by his wounds is off the card.
            .filter(|&f| world.comp::<Job>(f).is_none_or(|j| crate::systems::combat::unfit(world, f, j).is_none()))
            .map(|f| (crate::systems::combat::power(world, f), f, last_bout(f)))
            .collect();
        fighters.sort_by(|a, b| a.2.cmp(&b.2).then(b.0.total_cmp(&a.0)).then(a.1.cmp(&b.1)));
        if fighters.len() < 2 || !open(world, pit) || !standing(world, pit) {
            let owner = world.owner_of(pit);
            for (who, stake, _) in bets {
                if world.has::<Wallet>(who) {
                    ownership::refund(world, who, owner, stake, Flow::Gamble);
                    ownership::credit(world, pit, -stake);
                    book_take(world, pit, -stake, true);
                    // Review fix: the bet's `SetLeisureTax` surcharge comes back, as `on_abort`.
                    refund_surcharge(world, owner, stake);
                }
            }
            continue;
        }
        let mut rng = world.rng.word(WordNs::Bout, day, u64::from(pit.index));
        // Round 6 V6 (OPEN-217): the night's card, a bout for every
        // `bout_card_per` Fighters on shift here (at least one), paired in
        // the order above; the bets ride on the first (the main event).
        let per = usize::from(world.config.leisure.bout_card_per.max(2));
        let card = (fighters.len() / per).max(1).min(fighters.len() / 2);
        for k in 0..card {
            let (a, b) = (fighters[2 * k].1, fighters[2 * k + 1].1);
            let (fa, fb) = (fighters[2 * k].0, fighters[2 * k + 1].0);
            let (a_wins, hurt) = fight_bout(world, &mut rng, a, b, fa, fb);
            let (winner, loser) = if a_wins { (a, b) } else { (b, a) };
            // Round 7 W6 (OPEN-217): the stakes: the winner's purse from the
            // house (the main event's with its share of the stakes), the
            // loser's hurt (V15: no purse, no hurt, no loser's line).
            let stakes: i64 = if k == 0 { bets.iter().map(|&(_, s, _)| s).sum() } else { 0 };
            let purse = bout_purse(world, pit, winner, stakes);
            let stake_line = stakes_line(world, loser, purse, hurt);
            if k > 0 {
                let text = format!(
                    "{} beat {} at {} on the undercard{stake_line}",
                    world.name_of(winner),
                    world.name_of(loser),
                    world.name_of(pit)
                );
                world.push_event(EventKind::Bout, &[winner, loser, pit], text);
                continue;
            }
            let mut paid_out = 0;
            for &(who, stake, _) in &bets {
                // Lives at work phase 1 (§ 2.1): the bettors are the bout's served.
                crate::systems::service::arrive(world, who, pit, crate::components::ServeKind::Attendee, stake);
                let courage = world.comp::<Personality>(who).map_or(0.5, |p| p.courage);
                let backs_a = rng.random::<f32>() < 0.3 + 0.4 * courage;
                if backs_a == a_wins && world.has::<Wallet>(who) {
                    paid_out += house_pays(world, pit, who, 2 * stake, 2 * stake);
                }
            }
            let text = format!(
                "{} beat {} at {} (bets paid {paid_out}){stake_line}",
                world.name_of(winner),
                world.name_of(loser),
                world.name_of(pit)
            );
            world.push_event(EventKind::Bout, &[winner, loser, pit], text);
        }
    }
    // Round 6 V6: the book keeps the living Fighters only.
    crate::systems::vice::keep_bouts(world);
}

/// One bout of the card: the combat model C2 (C1's wrong-read c) runs it
/// through `combat::resolve_now` as a `Bout` (both subdue, bare hands, no
/// fight's memories or edges): the wounds by region land on the body and
/// the loser who was knocked out or took a Wound or worse was beaten (its
/// energy and safety at `bout_hurt`; the wounds send it to the patch-up,
/// `customers::hurt`), else he lost on points (`bout_points_tired`). Both
/// remember the `Bout`; the pit's book (round 6 V6). Returns whether `a`
/// won and the loser's hurt (0: on points). Fighters are adults (the
/// card's `ages::is_protected` gate); `resolve_fight_mods`'s backstop here.
fn fight_bout(world: &mut World, _rng: &mut impl Rng, a: EntityId, b: EntityId, _fa: f32, _fb: f32) -> (bool, f32) {
    if crate::systems::ages::is_protected(world, a) || crate::systems::ages::is_protected(world, b) {
        return (true, 0.0);
    }
    let mods = crate::systems::law::FightMods { why: crate::systems::combat::Purpose::Bout, ..Default::default() };
    let out = crate::systems::combat::resolve_now(world, a, b, mods);
    let a_wins = out.winner == a;
    let (winner, loser) = (out.winner, out.loser);
    let ls = if out.sides[0].id == loser { &out.sides[0] } else { &out.sides[1] };
    let beaten = matches!(ls.end, crate::systems::combat::End::KnockedOut | crate::systems::combat::End::Downed)
        || ls.took.iter().any(|t| t.1 >= crate::components::Severity::Wound);
    let cfg = &world.config.leisure;
    let (energy, hurt) = if beaten {
        let h = cfg.bout_hurt.clamp(0.0, 1.0);
        (h, h)
    } else {
        (cfg.bout_points_tired, 0.0)
    };
    if let Some(n) = world.comp_mut::<Needs>(loser) {
        n.energy = (n.energy - energy).max(0.0);
        n.safety = (n.safety - hurt).max(0.0);
    }
    if hurt > 0.0 {
        ownership::jobs2_row(world).bout_hurt += 1;
    }
    // L2 shadow fixes item 9: a bout is sanctioned, not an assault.
    world.remember(winner, MemoryKind::Bout, Some(loser), 0.4, 0.3, false);
    world.remember(loser, MemoryKind::Bout, Some(winner), 0.5, -0.4, false);
    crate::systems::vice::note_bout(world, a);
    crate::systems::vice::note_bout(world, b);
    (a_wins, hurt)
}

/// Round 7 W6 (OPEN-217): the bout's purse to `winner`: `[leisure]
/// bout_purse` plus `bout_purse_share` of the main event's `stakes`. Round 8
/// X5: the house's money, not a gambling win: the pit's owner pays it
/// (`Flow::Purse`: a corp or the city in full, an agent or a gang what it
/// holds) off the table's take (`book_take`: a front's week to its gang is
/// short by it). Returns the coins paid.
fn bout_purse(world: &mut World, pit: EntityId, winner: EntityId, stakes: i64) -> i64 {
    let cfg = &world.config.leisure;
    let want = cfg.bout_purse + (cfg.bout_purse_share * stakes as f32).round() as i64;
    if want <= 0 || !world.has::<Wallet>(winner) {
        return 0;
    }
    let owner = world.owner_of(pit);
    let paid = ownership::charge(world, owner, Some(winner), want, Flow::Purse);
    if paid > 0 {
        book_take(world, pit, -paid, true);
        ownership::credit(world, pit, -paid);
        let row = ownership::jobs2_row(world);
        row.bout_purses += 1;
        row.bout_purse_coins += paid as u32;
    }
    paid
}

/// Round 7 W6: the bout line's stakes: the purse and the loser's hurt
/// ("; purse 14c; Y hurt, to have it seen to"); round 8 X5: a loss that was
/// no beating "; Y lost on points".
fn stakes_line(world: &World, loser: EntityId, purse: i64, hurt: f32) -> String {
    let mut s = String::new();
    if purse > 0 {
        s.push_str(&format!("; purse {purse}c"));
    }
    let cfg = &world.config.leisure;
    if hurt > 0.0 {
        let how = if hurt >= cfg.bout_hurt_bad { "badly hurt" } else { "hurt" };
        s.push_str(&format!("; {} {how}, to have it seen to", world.name_of(loser)));
    } else if cfg.bout_hurt > 0.0 || cfg.bout_hurt_margin > 0.0 {
        s.push_str(&format!("; {} lost on points", world.name_of(loser)));
    }
    s
}
