use super::*;

/// C3a: a fight in the cells, for its tail.
#[derive(Clone, Debug)]
pub struct CellFight {
    pub jail: EntityId,
    pub why: String,
    pub place: String,
}

/// C3a, a cell fight's tail: the grudge, the line, a killing answered
/// inside, the crime in the cells (Round 8 X1, `OPEN-246`: through the
/// crime path; the starter who fell is charged with nothing).
pub(crate) fn cell_fight_end(world: &mut World, x: &CellFight, c: &crate::systems::combat::Clash) {
    let (p, m, jail) = (c.a, c.b, x.jail);
    let (winner, loser, died) = (c.winner, c.loser, c.died);
    if died && loser == p {
        crate::systems::demography::defended(world, p);
    }
    if living(world, loser) {
        crate::systems::grudges::add(world, loser, winner, GrudgeCause::Assaulted, 0.4, 0);
    }
    let end = if died { format!(", {} died", world.name_of(loser)) } else { String::new() };
    let text = format!(
        "{} went for {} in {} ({}); {} won{end}",
        world.name_of(p),
        world.name_of(m),
        x.place,
        x.why,
        world.name_of(winner)
    );
    world.push_event(EventKind::Cells, &[p, m, jail], text);
    // Round 9 Z1: a grudge over a killing answered inside.
    crate::systems::hunt::cell_strike(world, p, m, winner, died, jail);
    if living(world, p) {
        let murder = died && loser == m;
        let crime = if murder { Crime::Murder } else { Crime::Assault };
        let tile = world.comp::<Position>(p).map_or_else(Default::default, |x| x.tile);
        crate::systems::law::raise_crime_on(world, p, (!murder).then_some(m), Some(m), crime, tile);
    }
}

/// C3a: a blackmailed culprit's fight to silence the witness, for its tail.
#[derive(Clone, Debug)]
pub struct Silence {
    pub w: EntityId,
    pub culprit: EntityId,
    pub crime: Crime,
    pub line: String,
    pub actors: Vec<EntityId>,
    pub tile: crate::components::TilePos,
}

/// C3a, the silencing's tail: the line, the crime, the witness's grudge and,
/// a witness who won, the report.
pub(crate) fn silence_end(world: &mut World, x: &Silence, c: &crate::systems::combat::Clash) {
    let (w, culprit) = (x.w, x.culprit);
    let (winner, loser, died) = (c.winner, c.loser, c.died);
    let (wn, cn) = (world.name_of(w), world.name_of(culprit));
    if died && loser == culprit {
        crate::systems::demography::defended(world, culprit);
    }
    let end = if died {
        format!(", {} died", world.name_of(loser))
    } else if winner == w {
        format!("; {wn} won and went to the law")
    } else {
        format!("; {cn} won and {wn} will keep quiet now")
    };
    world.push_event(EventKind::Blackmail, &x.actors, format!("{}{end}", x.line));
    ownership::jobs2_row(world).bm_silenced += 1;
    if living(world, culprit) {
        let murder = died && loser == w;
        let kind = if murder { Crime::Murder } else { Crime::Assault };
        if murder {
            world.push_event(EventKind::Murder, &[culprit, w], format!("{cn} killed {wn}"));
        }
        crate::systems::law::raise_crime_on(world, culprit, (!murder).then_some(w), Some(w), kind, x.tile);
    }
    if living(world, w) {
        crate::systems::grudges::add(world, w, culprit, GrudgeCause::Assaulted, 0.5, 0);
        if winner == w && living(world, culprit) {
            crate::systems::law::file_report(world, x.crime, culprit, Some(w));
        }
    }
}

/// The demand (a `Visit` arrived, or a Statistical one at its hour).
pub fn blackmailed(world: &mut World, w: EntityId, culprit: EntityId, walked: bool) {
    let Some(c) = cfg(world).cloned() else { return };
    let Some(i) = world.consequence.blackmail.iter().position(|b| b.witness == w && b.culprit == culprit) else {
        return;
    };
    let b = world.consequence.blackmail[i].clone();
    // Round 8 X1: a demand by a sleeper (0 is right; `arrivals` waits).
    if walked && crate::systems::presence::asleep(world, w) {
        ownership::jobs2_row(world).asleep_acts += 1;
    }
    let purse = world.purse(Some(culprit)).max(0);
    let demand = ((purse as f32 * c.bm_demand_share).round() as i64).clamp(c.bm_demand_min, c.bm_demand_max);
    let (here, place) = place_of(world, culprit);
    let (wn, cn) = (world.name_of(w), world.name_of(culprit));
    let how = if walked { format!("found {cn}") } else { format!("caught {cn} (off screen)") };
    let again = if b.demands > 0 { " again" } else { "" };
    let (he, him, _) = pro(world, w);
    let head =
        format!("{wn} {how} {place}{again}: {he} saw the {} and wants {demand}c to keep it quiet", b.crime.label());
    let mut actors = vec![w, culprit];
    actors.extend(here);
    ownership::jobs2_row(world).bm_demands += 1;
    // M18 P5 (P3's offer shape): the player's body (driven or on autopilot)
    // is asked; the human pays, refuses or goes for the witness
    // (`blackmail_answer`), and a demand let lapse is refused.
    if crate::systems::player::possessed(world, culprit) {
        let until = world.tick + world.config.slice.blackmail_offer_minutes;
        let today = world.day();
        if let Some(x) = world.consequence.blackmail.get_mut(i) {
            x.next_day = today + 1;
        }
        let line = format!("{wn} saw the {} and wants {demand}c to keep it quiet", b.crime.label());
        world.push_event(EventKind::Blackmail, &actors, format!("{head}: the answer is {cn}'s"));
        let what = crate::components::OfferKind::Blackmail { witness: w, demand };
        crate::systems::player::offer(world, culprit, what, until, line);
        return;
    }
    let t = traits(world, culprit);
    let p_pay = (c.bm_pay_base + c.bm_pay_fear * (1.0 - t.courage) + c.bm_pay_lawful * t.lawful
        - c.bm_pay_pride * t.pride)
        .clamp(0.0, 1.0);
    let draw = world.day() ^ key(w);
    if purse >= demand && hash(world, P_PAY, culprit, draw) < p_pay {
        return bm_pay(world, &c, i, demand, &actors, &head);
    }
    let why = if purse < demand { "had nothing like it".to_string() } else { format!("told {him} where to go") };
    let silence = hash(world, P_SILENCE, culprit, draw) < c.bm_silence_w * (1.0 - t.lawful) * (0.5 + t.courage);
    bm_refuse(world, &c, i, actors, &head, &why, silence);
}

/// A demand paid: the next one in `bm_interval_days`.
fn bm_pay(world: &mut World, c: &ConsequenceCfg, i: usize, demand: i64, actors: &[EntityId], head: &str) {
    let b = world.consequence.blackmail[i].clone();
    let (w, culprit) = (b.witness, b.culprit);
    let paid = ownership::pay(world, Some(culprit), Some(w), demand, Flow::Bribe);
    let today = world.day();
    if let Some(x) = world.consequence.blackmail.get_mut(i) {
        x.demands += 1;
        x.tries = 0;
        x.paid += paid;
        x.next_day = today + u64::from(c.bm_interval_days);
    }
    crate::systems::grudges::add(world, culprit, w, GrudgeCause::Robbed, 0.3, 0);
    let cn = world.name_of(culprit);
    world.push_event(EventKind::Blackmail, actors, format!("{head}: {cn} paid {paid}c"));
    let row = ownership::jobs2_row(world);
    row.bm_paid += 1;
    row.bm_coins += paid;
}

/// A demand refused: the culprit goes for the witness (`silence`), or the
/// witness goes to the law.
fn bm_refuse(
    world: &mut World,
    c: &ConsequenceCfg,
    i: usize,
    actors: Vec<EntityId>,
    head: &str,
    why: &str,
    silence: bool,
) {
    let b = world.consequence.blackmail.remove(i);
    let (w, culprit) = (b.witness, b.culprit);
    let (wn, cn) = (world.name_of(w), world.name_of(culprit));
    let (_, him, _) = pro(world, w);
    if silence {
        let tile = world.comp::<Position>(w).map_or_else(Default::default, |p| p.tile);
        let mods = crate::systems::law::FightMods { kill_mult: c.bm_kill_mult, ..Default::default() };
        // C3a: live when both are Full; the tail is `silence_end`.
        let line = format!("{head}: {cn} {why} and went for {him}");
        let tail = Silence { w, culprit, crime: b.crime, line, actors, tile };
        crate::systems::combat::fight(world, culprit, w, mods, crate::systems::combat::Tail::Silence(Box::new(tail)));
        return;
    }
    crate::systems::law::file_report(world, b.crime, culprit, Some(w));
    crate::systems::grudges::add(world, culprit, w, GrudgeCause::Betrayed, c.snitch_grudge, 0);
    world.push_event(EventKind::Blackmail, &actors, format!("{head}: {cn} {why}; {wn} went to the law"));
    ownership::jobs2_row(world).bm_reported += 1;
}

/// M18 P5: the human's answer to a witness's demand made as an offer
/// (`blackmailed`): pay (`0`), refuse (`1`: he goes to the law; a lapsed
/// offer too) or go for him (`2`: the silencing fight). `Err`: why not.
pub fn blackmail_answer(
    world: &mut World,
    w: EntityId,
    culprit: EntityId,
    demand: i64,
    answer: u8,
) -> Result<(), String> {
    let Some(c) = cfg(world).cloned() else { return Err("no blackmail in this city".into()) };
    let Some(i) = world.consequence.blackmail.iter().position(|b| b.witness == w && b.culprit == culprit) else {
        return Err("the witness is gone".into());
    };
    let crime = world.consequence.blackmail[i].crime;
    let (wn, cn) = (world.name_of(w), world.name_of(culprit));
    let head = format!("{wn} wanted {demand}c to keep {cn}'s {} quiet", crime.label());
    let actors = vec![w, culprit];
    match answer {
        0 => {
            if world.purse(Some(culprit)) < demand {
                return Err(format!("you have less than {demand}c"));
            }
            bm_pay(world, &c, i, demand, &actors, &head);
        }
        2 => {
            if !living(world, w) {
                return Err("the witness is gone".into());
            }
            let (_, him, _) = pro(world, w);
            bm_refuse(world, &c, i, actors, &head, &format!("told {him} where to go"), true);
        }
        _ => {
            let (_, him, _) = pro(world, w);
            bm_refuse(world, &c, i, actors, &head, &format!("told {him} where to go"), false);
        }
    }
    Ok(())
}
