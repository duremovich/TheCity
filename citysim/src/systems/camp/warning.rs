use super::*;

/// Round 4 T1 (OPEN-231, Dylan 2026-10-27: neglect warns first): the
/// service at `home`'s door over `kids`' unfed days. Each child's
/// `warned_day` is today and its unfed count starts again; the parents
/// hear it (one `Custody` event, the grace and the window in it); the
/// first free parent short of the grace's food money (`warn_grace_days`
/// of the children's food at his meal price) asks his Friends for it
/// (`warn_ask_*`, an edge debt), and the line says how it went.
pub(super) fn warn(world: &mut World, home: EntityId, kids: &[EntityId], day: u32) {
    let cfg = world.config.camp.clone();
    let mut parents: Vec<EntityId> = Vec::new();
    let mut unfed = 0u8;
    for &k in kids {
        if let Some(c) = world.comp_mut::<Child>(k) {
            unfed = unfed.max(c.hunger_days);
            c.warned_day = Some(day);
            c.hunger_days = 0;
        }
        // Round 5 U2 (`reporter_57837 d27`): the parents living at this door
        // hear it (a parent elsewhere was named at a Block not his).
        parents.extend(free_parents_at(world, k, home));
    }
    parents.sort_unstable();
    parents.dedup();
    ownership::jobs2_row(world).camp_warnings += kids.len() as u32;
    let names: Vec<String> = kids
        .iter()
        .map(|&k| format!("{} ({} y)", world.name_of(k), world.comp::<Identity>(k).map_or(0, |i| i.age_years())))
        .collect();
    let who: Vec<String> = parents.iter().map(|&p| world.name_of(p)).collect();
    let heard = if who.is_empty() {
        "nobody was home to hear it".to_string()
    } else {
        format!("{} heard it", who.join(" and "))
    };
    let until = day + u32::from(cfg.warn_repeat_days);
    let mut text = format!(
        "the child protective service came to the door of {}: {} went {unfed} days unfed; {heard}: {} days' grace, and unfed again before d{until} the service takes them",
        world.name_of(home),
        names.join(", "),
        cfg.warn_grace_days,
    );
    let mut actors: Vec<EntityId> = parents.clone();
    if let Some(asker) = parents.iter().copied().find(|&p| {
        world.has::<crate::components::Brain>(p)
            && !world.has::<crate::components::Sentence>(p)
            && crate::systems::demography::is_adult(world, p)
            && crate::systems::law::living(world, p)
    }) {
        // Round 6 V1: the household's children (a warning is per household).
        let kids_home = kids.len().max(1) as f32;
        let units = (CHILD_FOOD_PER_DAY * kids_home * f32::from(cfg.warn_grace_days)).ceil() as i64;
        let cost = units * crate::systems::life::meal_price(world, asker);
        let held: i64 =
            parents.iter().map(|&p| world.comp::<crate::components::Wallet>(p).map_or(0, |w| w.coins.max(0))).sum();
        if held >= cost {
            text.push_str(&format!(" ({held}c between them covers the {cost}c of food)"));
        } else {
            let (bits, lenders) = warn_ask(world, asker, cost - held, &cfg, day);
            actors.extend(lenders);
            text.push_str(&format!("; {} has {held}c of the {cost}c of food and {bits}", world.name_of(asker)));
        }
    }
    actors.extend(kids.iter().copied());
    actors.push(home);
    world.push_event(EventKind::Custody, &actors, text);
}

/// Round 4 T1: the warned parent's ladder of Friends (care's ask, round 3
/// S4, with `[camp] warn_ask_*`): the line's tail and the Friends asked.
fn warn_ask(
    world: &mut World,
    id: EntityId,
    mut need: i64,
    cfg: &crate::config::CampCfg,
    day: u32,
) -> (String, Vec<EntityId>) {
    use crate::components::{MemoryKind, RelKind, Wallet};
    let mut friends: Vec<(f32, EntityId)> = world
        .neighbours(id)
        .filter(|&o| o != id && crate::systems::law::living(world, o))
        .filter_map(|o| {
            world
                .edge(id, o)
                .filter(|e| e.kind == RelKind::Friend && e.affinity >= cfg.warn_ask_affinity)
                .map(|e| (e.affinity, o))
        })
        .collect();
    friends.sort_by(|p, q| q.0.total_cmp(&p.0).then(p.1.cmp(&q.1)));
    friends.truncate(usize::from(cfg.warn_ask_tries.max(1)));
    if friends.is_empty() {
        return ("no friend to ask".to_string(), Vec::new());
    }
    let mut bits: Vec<String> = Vec::new();
    let mut asked = Vec::new();
    for (k, (aff, f)) in friends.into_iter().enumerate() {
        if need <= 0 {
            break;
        }
        asked.push(f);
        let fname = world.name_of(f);
        let spare = world.comp::<Wallet>(f).map_or(0, |w| w.coins) - cfg.warn_ask_keep;
        if spare <= 0 {
            bits.push(format!("{fname} had nothing to spare"));
            continue;
        }
        let greed = world.comp::<Personality>(f).map_or(0.5, |p| p.greed);
        let roll = hash(world, PURPOSE_WARN_ASK, key(id), u64::from(day) * 8 + k as u64);
        if roll >= aff * (1.0 - greed / 2.0) {
            world.remember(id, MemoryKind::Rejected, Some(f), 0.5, -0.4, false);
            bits.push(format!("{fname} turned them down"));
            continue;
        }
        let lend = spare.min(need);
        if let Some(w) = world.comp_mut::<Wallet>(f) {
            w.coins -= lend;
        }
        if let Some(w) = world.comp_mut::<Wallet>(id) {
            w.coins += lend;
        }
        need -= lend;
        let (lo, _) = crate::components::edge_key(id, f);
        let tick = world.tick;
        let e = world.edge_entry(id, f);
        e.debt += if lo == id { lend as i32 } else { -(lend as i32) };
        e.debt_since.get_or_insert(tick);
        e.last_interaction = tick;
        world.remember(f, MemoryKind::Paid, Some(id), 0.5, 0.1, false);
        world.remember(id, MemoryKind::Paid, Some(f), 0.6, 0.3, false);
        bits.push(format!("{fname} lent {lend}c"));
    }
    let tail = if need <= 0 { ": covered".to_string() } else { format!(": still {need}c short") };
    (format!("asked friends for the rest: {}{tail}", bits.join("; ")), asked)
}

/// Round 4 T1 (OPEN-231): a child living in `home` is on the service's
/// warning (inside `warn_repeat_days` of it): the household's grocery run
/// fills the children's food from the pantry alone (`routine::grocery_plan`,
/// `lod::stat_grocery`: food carried for a shift feeds no child).
pub fn home_on_warning(world: &World, home: EntityId) -> bool {
    let window = u32::from(world.config.camp.warn_repeat_days);
    if window == 0 {
        return false;
    }
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    world
        .residents_of(home)
        .iter()
        .any(|&r| world.comp::<Child>(r).and_then(|c| c.warned_day).is_some_and(|w| day.saturating_sub(w) <= window))
}

/// Round 6 V1 (OPEN-231): `id`'s household at `home` has a child on the
/// service's warning (in a shared Home his household's children only, the
/// Home's otherwise): his grocery run reads the pantry alone.
pub fn household_on_warning(world: &World, id: EntityId, home: EntityId) -> bool {
    let k = crate::systems::shelf::key(world, id);
    let shared = crate::systems::shelf::claim(world, home, k).is_some();
    household_on_warning_in(world, id, home, shared)
}

/// [`household_on_warning`] with the shelf's sharing read by the caller
/// (`shelf::Shelf::shared`; the view milestone V2b).
pub fn household_on_warning_in(world: &World, id: EntityId, home: EntityId, shared: bool) -> bool {
    let k = crate::systems::shelf::key(world, id);
    if !shared {
        return home_on_warning(world, home);
    }
    let window = u32::from(world.config.camp.warn_repeat_days);
    if window == 0 {
        return false;
    }
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    world.residents_of(home).iter().any(|&r| {
        crate::systems::shelf::key(world, r) == k
            && world.comp::<Child>(r).and_then(|c| c.warned_day).is_some_and(|w| day.saturating_sub(w) <= window)
    })
}

/// Round 4 T1 (OPEN-231): the pantry units of `home` an adult may eat or
/// pack: on the service's warning the children's food
/// (`ages::children_food_need`) is kept back for the midnight meal (a
/// Block's adults ate the pantry bare before the children's turn); else
/// the whole stock.
///
/// Round 6 V1 (OPEN-231): read for adult `id`. In a shared Home (two
/// households or more under one roof, a stake on the shelf) his
/// household's claim and what the other households hold above their
/// children's food (`shelf::adult_reach`: a neighbour's crust is eaten,
/// his child's meal is not; the Block's adults who never shopped ate the
/// food a family bought for its children, 186 of 372 second-warning takes
/// at 20k x 30 in households whose purse covered the food), less his own
/// household's children's food on its warning.
///
/// Round 7 W1 (OPEN-243): an adult who holds a meal's coins
/// (`life::can_buy_meal`) keeps the children's food back too, warned or
/// not: he buys his own meal rather than eat his children's (a jobless
/// father bought the children's two units by day and the household's
/// adults ate them by the evening, 628 of 1,008 unfed household-nights at
/// 20k x 20 had a free parent holding 6c or more). The broke still eat it.
pub fn adults_share(world: &World, id: EntityId, home: EntityId) -> u32 {
    if let Some(c) = crate::systems::shelf::adult_reach(world, id, home) {
        let mine = (c + 1e-3).floor() as u32;
        if !household_on_warning(world, id, home) && !keeps_back(world, id) {
            return mine;
        }
        return mine.saturating_sub(crate::systems::shelf::kids_need(world, id, home));
    }
    let stock = world.comp::<Building>(home).map_or(0, |b| b.stock_food);
    if !home_on_warning(world, home) && !keeps_back(world, id) {
        return stock;
    }
    stock.saturating_sub(crate::systems::ages::children_food_need(world, home))
}

/// Round 7 W1: `id` buys his own meal rather than eat the children's food.
fn keeps_back(world: &World, id: EntityId) -> bool {
    crate::systems::ages::on(world) && crate::systems::life::can_buy_meal(world, id)
}

/// Round 4 T1 (OPEN-231): `p` can have his Camp children home: alive,
/// adult, free, in a standing Home whose pantry holds `home_pantry_days` of
/// the children's food with his `away` Camp children counted in. The Home.
fn home_holds(world: &World, p: EntityId, away: usize, cfg: &crate::config::CampCfg) -> Option<EntityId> {
    if !crate::systems::law::living(world, p)
        || !crate::systems::demography::is_adult(world, p)
        || world.has::<crate::components::Sentence>(p)
    {
        return None;
    }
    let h = world.comp::<Household>(p).and_then(|x| x.home)?;
    world.comp::<Building>(h).filter(|b| b.kind == BuildingKind::Home && !b.demolished && !b.derelict)?;
    // Round 6 V1 (OPEN-231): his household's children and claim in a shared Home.
    let k = crate::systems::shelf::key(world, p);
    let shared = crate::systems::shelf::claim(world, h, k).is_some();
    let at_home = world
        .residents_of(h)
        .iter()
        .filter(|&&r| world.has::<Child>(r) && (!shared || crate::systems::shelf::key(world, r) == k))
        .count();
    let kids = (at_home + away) as f32;
    let held = crate::systems::shelf::held_by(world, p, h) as f32;
    (held >= CHILD_FOOD_PER_DAY * kids * cfg.home_pantry_days).then_some(h)
}

/// Round 4 T1 (OPEN-231): midnight, before the ladder: each Camp child's
/// let-home count (a parent's Home held the food of him and his other Camp
/// children tonight: one more, else none); at `let_home_days` the service
/// lets the parent's children home together, on a warning (`warned_day`
/// today: unfed again inside the window and the service takes them back).
pub(super) fn let_home_daily(world: &mut World) {
    let cfg = world.config.camp.clone();
    if cfg.let_home_days == 0 {
        return;
    }
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    // (child, its camp, its parents), every Camp child.
    let mut kids: Vec<(EntityId, EntityId, Vec<EntityId>)> = Vec::new();
    for camp in standing_camps(world) {
        for &k in children_of(world, camp) {
            kids.push((k, camp, parents_of(world, k)));
        }
    }
    let mut away_n: std::collections::BTreeMap<EntityId, usize> = std::collections::BTreeMap::new();
    for (_, _, ps) in &kids {
        for &p in ps {
            *away_n.entry(p).or_default() += 1;
        }
    }
    let away = |p: EntityId| away_n.get(&p).copied().unwrap_or(0);
    let picks: Vec<(EntityId, EntityId, Option<EntityId>)> = kids
        .iter()
        .map(|(k, camp, ps)| (*k, *camp, ps.iter().copied().find(|&p| home_holds(world, p, away(p), &cfg).is_some())))
        .collect();
    // (parent, camp) -> the children going home and the nights held.
    let mut going: std::collections::BTreeMap<(EntityId, EntityId), (Vec<EntityId>, u16)> =
        std::collections::BTreeMap::new();
    for (k, camp, pick) in picks {
        let Some(c) = world.comp_mut::<Child>(k) else { continue };
        if pick.is_some() {
            c.home_fed_days = c.home_fed_days.saturating_add(1);
        } else {
            c.home_fed_days = 0;
        }
        if let Some(p) = pick.filter(|_| c.home_fed_days >= cfg.let_home_days) {
            let n = c.home_fed_days;
            let e = going.entry((p, camp)).or_default();
            e.0.push(k);
            e.1 = e.1.max(n);
        }
    }
    for ((p, camp), (ks, n)) in going {
        let mut at = None;
        for &k in &ks {
            release(world, k, ReleaseWhy::LetHome);
            at = send_home(world, k, p);
            if let Some(c) = world.comp_mut::<Child>(k) {
                c.warned_day = Some(day);
                c.home_fed_days = 0;
                c.hunger_days = 0;
            }
        }
        let names: Vec<String> = ks.iter().map(|&k| world.name_of(k)).collect();
        let (pn, what) = (world.name_of(p), world.name_of(camp));
        let to = at.map_or_else(|| "the street".to_string(), |h| world.name_of(h));
        let mut actors = ks.clone();
        actors.extend([p, camp]);
        actors.extend(at);
        world.push_event(
            EventKind::Custody,
            &actors,
            format!(
                "the service let {} go home from {what} to {pn} at {to}: the pantry has held their food {n} nights running (home on a warning: unfed again before d{} and they go back)",
                names.join(", "),
                day + u32::from(cfg.warn_repeat_days)
            ),
        );
        ownership::jobs2_row(world).camp_let_home += ks.len() as u32;
    }
}
