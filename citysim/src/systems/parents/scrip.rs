use super::*;

/// A holder's scrip of `issuer`.
pub fn scrip_of(world: &World, agent: EntityId, issuer: OutsideId) -> i64 {
    world.outside.scrip.get(&agent).map_or(0, |v| v.iter().filter(|&&(i, _)| i == issuer).map(|&(_, n)| n).sum())
}

fn add_scrip(world: &mut World, agent: EntityId, issuer: OutsideId, delta: i64) {
    let v = world.outside.scrip.entry(agent).or_default();
    match v.iter_mut().find(|(i, _)| *i == issuer) {
        Some(e) => e.1 += delta,
        None => v.push((issuer, delta)),
    }
    v.retain(|&(_, n)| n > 0);
    if v.is_empty() {
        world.outside.scrip.remove(&agent);
    }
    if let Some(p) = parent_mut(world, issuer) {
        p.scrip_out = (p.scrip_out + delta).max(0);
    }
}

/// Scrip's rate: par while the issuer's health holds `scrip_par_min`, else its health.
fn scrip_rate(world: &World, issuer: OutsideId) -> f32 {
    let par = world.config.outside.parents.scrip_par_min;
    parent(world, issuer).map_or(0.0, |p| if p.health >= par { 1.0 } else { p.health.max(0.0) })
}

/// The issuer whose scrip a wage from `payer` pays: a corp branch of a
/// living megacorp parent, scrip on and not banned.
fn issuer_of(world: &World, payer: Option<EntityId>) -> Option<OutsideId> {
    let cfg = &world.config.outside.parents;
    if !cfg.scrip_enabled || world.outside.scrip_banned || !parents_on(world) {
        return None;
    }
    let p = payer.and_then(|x| world.comp::<Corp>(x)).and_then(|c| c.parent)?;
    world.outside.faction(p).filter(|f| f.kind == OutsideKind::Megacorp && !f.dead && f.parent.is_some()).map(|f| f.id)
}

/// `ownership::transfer`'s wage hook: `scrip_wage_share` of a wage `from` a
/// parented corp branch `to` an agent is scrip; its coins cross from the
/// agent to the parent at issue.
pub fn scrip_wage(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, moved: i64) {
    let Some(issuer) = issuer_of(world, from) else { return };
    let Some(a) = to.filter(|&a| world.has::<crate::components::Wallet>(a)) else { return };
    let share = (f64::from(world.config.outside.parents.scrip_wage_share) * moved as f64).round() as i64;
    if share <= 0 {
        return;
    }
    let crossed = ownership::cross_out(world, Some(a), issuer, share, Flow::Scrip, false);
    if crossed <= 0 {
        return;
    }
    if let Some(f) = world.outside.faction_mut(issuer) {
        f.income_today -= crossed;
    }
    add_scrip(world, a, issuer, crossed);
    world.outside.today.scrip_paid += crossed;
}

/// `ownership::transfer`'s spending hook: an agent paying food or rent to a
/// building owner that is a branch of an issuer whose scrip it holds pays
/// scrip first: the parent redeems it into the agent's purse at its rate
/// (par, or health below `scrip_par_min`), then the coins move as usual.
pub fn scrip_spend(world: &mut World, from: Option<EntityId>, to: Option<EntityId>, amount: i64) {
    if amount <= 0 || world.outside.scrip.is_empty() {
        return;
    }
    let Some(a) = from else { return };
    let Some(issuer) = to.and_then(|o| world.comp::<Corp>(o)).and_then(|c| c.parent) else { return };
    let held = scrip_of(world, a, issuer);
    if held <= 0 || world.outside.faction(issuer).is_none_or(|f| f.dead) {
        return;
    }
    let rate = scrip_rate(world, issuer);
    if rate <= 0.0 {
        return;
    }
    let used = (((amount as f64) / f64::from(rate)).ceil() as i64).min(held);
    let coins = ((used as f64) * f64::from(rate)).floor() as i64;
    let moved = ownership::cross_in(world, issuer, Some(a), coins, Flow::Scrip);
    if moved <= 0 {
        return;
    }
    if let Some(f) = world.outside.faction_mut(issuer) {
        f.income_today += moved;
    }
    let spent = if moved < coins { ((moved as f64) / f64::from(rate)).ceil() as i64 } else { used };
    add_scrip(world, a, issuer, -spent.min(held));
    world.outside.today.scrip_spent += moved;
}

/// § 4 conversion at a Fixer (deviation: a daily pass, not a walk): a holder
/// with `convert_min` scrip and under three meals' coins converts at the
/// issuer's health, `fixer_cut` to the nearest open Fixer's owner (the
/// city's when none), both from the parent's treasury; one day in seven
/// per holder (`all` in tests).
pub fn convert_pass(world: &mut World, all: bool) {
    let cfg = world.config.outside.parents.clone();
    let day = world.day();
    let meal = world.mean_price().max(1);
    let fixer = crate::systems::contracts::open_fixers(world).first().copied();
    let owner = fixer.and_then(|f| world.owner_of(f));
    let holders: Vec<(EntityId, Vec<(OutsideId, i64)>)> =
        world.outside.scrip.iter().map(|(&a, v)| (a, v.clone())).collect();
    for (a, held) in holders {
        if !all && !(u64::from(a.index) + day).is_multiple_of(7) {
            continue;
        }
        // M18 P5h (a probe's debug assert): a holder gone (no Wallet) is no purse to read.
        if !world.has::<crate::components::Wallet>(a) || world.purse(Some(a)) >= 3 * meal {
            continue;
        }
        for (issuer, units) in held {
            if units < cfg.convert_min || world.outside.faction(issuer).is_none_or(|f| f.dead) {
                continue;
            }
            let h = parent(world, issuer).map_or(0.0, |p| p.health.clamp(0.0, 1.0));
            let to_holder = (units as f64 * f64::from(h) * f64::from(1.0 - cfg.fixer_cut)).floor() as i64;
            let to_fixer = (units as f64 * f64::from(h) * f64::from(cfg.fixer_cut)).floor() as i64;
            let m1 = ownership::cross_in(world, issuer, Some(a), to_holder, Flow::Scrip);
            let m2 = ownership::cross_in(world, issuer, owner, to_fixer, Flow::Scrip);
            if let Some(f) = world.outside.faction_mut(issuer) {
                f.income_today += m1 + m2;
            }
            add_scrip(world, a, issuer, -units);
            world.outside.today.scrip_converted += m1;
        }
    }
}

/// Every holder's scrip of `issuer` redeemed at `rate` (or wiped at 0).
pub(super) fn redeem_all(world: &mut World, issuer: OutsideId, rate: f32) -> (i64, i64) {
    let holders: Vec<(EntityId, i64)> = world
        .outside
        .scrip
        .iter()
        .filter_map(|(&a, v)| v.iter().find(|&&(i, _)| i == issuer).map(|&(_, n)| (a, n)))
        .collect();
    let (mut paid, mut units) = (0, 0);
    for (a, n) in holders {
        let coins = (n as f64 * f64::from(rate)).floor() as i64;
        if coins > 0 && world.is_alive(a) {
            let m = ownership::cross_in(world, issuer, Some(a), coins, Flow::Scrip);
            if let Some(f) = world.outside.faction_mut(issuer) {
                f.income_today += m;
            }
            paid += m;
        }
        units += n;
        add_scrip(world, a, issuer, -n);
    }
    (paid, units)
}

/// § 4 the wipe: every holder's scrip of `issuer` gone; each district's
/// unrest rises by `scrip_unrest × wiped there ÷ its residents' coins`.
pub(super) fn wipe_scrip(world: &mut World, issuer: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let holders: Vec<(EntityId, i64)> = world
        .outside
        .scrip
        .iter()
        .filter_map(|(&a, v)| v.iter().find(|&&(i, _)| i == issuer).map(|&(_, n)| (a, n)))
        .collect();
    if holders.is_empty() {
        return;
    }
    let mut by_district: BTreeMap<usize, i64> = BTreeMap::new();
    let mut total = 0;
    for (a, n) in holders {
        if let Some(h) = world.comp::<crate::components::Household>(a).and_then(|h| h.home) {
            *by_district.entry(world.district_of_building(h).index()).or_insert(0) += n;
        }
        total += n;
        add_scrip(world, a, issuer, -n);
    }
    for (d, wiped) in by_district {
        let coins: i64 =
            world.districts.get(d).map_or(0, |x| x.residents.iter().map(|&r| world.purse(Some(r)).max(0)).sum());
        if let Some(x) = world.districts.get_mut(d) {
            x.unrest = (x.unrest + cfg.scrip_unrest * wiped as f32 / coins.max(1) as f32).min(1.0);
        }
    }
    world.outside.today.scrip_wiped += total;
    let name = world.outside.faction(issuer).map_or_else(String::new, |f| f.name.clone());
    world.push_event(EventKind::ScripWiped, &[], format!("{name}'s scrip is worthless: {total} units wiped"));
}

/// `BanScrip`: no issuance; every holder redeemed at its issuer's rate.
pub fn ban_scrip(world: &mut World, on: bool) -> Result<String, String> {
    world.outside.scrip_banned = on;
    if !on {
        return Ok("Scrip allowed again".to_string());
    }
    let issuers: Vec<OutsideId> = world.outside.parents().map(|f| f.id).collect();
    let mut paid = 0;
    for i in issuers {
        let rate = scrip_rate(world, i);
        paid += redeem_all(world, i, rate).0;
    }
    world.outside.today.scrip_spent += paid;
    Ok(format!("Scrip banned; {paid} redeemed"))
}
