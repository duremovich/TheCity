use super::*;

/// § 2 FoundBranch: a corp named for the brand (" II", " III" past a dead
/// one), its template's niches, `branch_seed` from abroad, a grace from
/// upkeep and, when a Lot and the coins allow, its first building. The next
/// Reinforce comes with no cooldown.
pub fn found_branch(world: &mut World, id: OutsideId) -> Option<EntityId> {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let f = world.outside.faction(id)?;
    if f.kind != OutsideKind::Megacorp || !corp_branches(world, id).is_empty() || f.treasury < cfg.branch_seed {
        return None;
    }
    let brand = f.parent.as_ref()?.branch_name.clone();
    let rows = world.config.corps.row_count();
    let row = (0..rows).find(|&i| world.config.corps.template_of(i) == brand)?;
    let niches = world.config.corps.niches_of(row);
    // M17 review: the numeral counts the branches this parent founded (a
    // living count repeated "II" after a death, and the 20k "Arasaka 2"
    // copies inflated it).
    let taken = f.parent.as_ref().map_or(0, |p| p.founded as usize);
    let numerals = ["II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    let name = match numerals.get(taken) {
        Some(n) => format!("{brand} {n}"),
        None => format!("{brand} {}", taken + 2),
    };
    if let Some(p) = parent_mut(world, id) {
        p.founded += 1;
    }
    let corp = ownership::spawn_corp(world, name.clone(), niches.clone(), 0, None);
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.parent = Some(id);
        c.treasury_ref = cfg.branch_seed.max(1);
        c.closing = 0;
        c.upkeep_grace_until = Some(now + u64::from(cfg.upkeep_grace_days) * TICKS_PER_DAY);
    }
    fund(world, id, corp, cfg.branch_seed);
    if let Some(c) = world.comp_mut::<Corp>(corp) {
        c.closing = c.treasury;
    }
    // The first building, on the lowest Lot, when the seed covers it.
    if let Some(&n) = niches.iter().next() {
        if let Some(kind) = crate::systems::corp_brain::build_kind_for(world, corp, n) {
            let cost = crate::systems::founding::found_cost(world, kind).unwrap_or(i64::MAX);
            if world.purse(Some(corp)) >= cost {
                if let Some(lot) = crate::systems::founding::vacant_lots(world).into_iter().next() {
                    if crate::systems::founding::build_on_lot(world, lot, kind, Some(corp)).is_ok() {
                        crate::systems::industry::pay_build(world, Some(corp), cost, Some(lot), true);
                    }
                }
            }
        }
    }
    if let Some(p) = parent_mut(world, id) {
        p.branches.push(corp);
        p.branches.sort_unstable();
        p.branch_died_at = None;
        p.last_reinforce = None;
    }
    world.outside.today.branches_founded += 1;
    let pname = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    world.push_event(EventKind::BranchFounded, &[corp], format!("{pname} founded {name} in the city"));
    Some(corp)
}

/// § 2: a local corp that has held `found_parent_treasury` and a niche
/// share of `found_parent_share` for `found_parent_days` founds a parent:
/// `found_parent_frac` of its treasury crosses abroad as the account's
/// treasury (nothing minted), its income and upkeep rates on it, the exec's
/// persona, exposure 0.5. At most `max_accounts` accounts.
pub fn found_parents(world: &mut World) {
    let cfg = world.config.outside.parents.clone();
    let mut streak = std::mem::take(&mut world.outside.found_streak);
    let mut due: Vec<EntityId> = Vec::new();
    for c in world.corps() {
        let Some(x) = world.comp::<Corp>(c).filter(|x| x.parent.is_none()) else { continue };
        let share = x.share_hist.values().filter_map(|v| v.back().copied()).fold(0.0f32, f32::max);
        if x.treasury >= cfg.found_parent_treasury && share >= cfg.found_parent_share {
            let n = streak.entry(c).or_insert(0);
            *n += 1;
            if *n > cfg.found_parent_days {
                due.push(c);
            }
        } else {
            streak.remove(&c);
        }
    }
    streak.retain(|&c, _| world.has::<Corp>(c));
    world.outside.found_streak = streak;
    for c in due {
        if world.outside.factions.len() >= cfg.max_accounts {
            break;
        }
        found_parent_for(world, c);
    }
}

/// A local corp founds its parent: `found_parent_frac` of its treasury
/// moved abroad into a new megacorp account it is the branch of. The
/// account's id, or `None` for no corp.
fn found_parent_for(world: &mut World, c: EntityId) -> Option<OutsideId> {
    let cfg = world.config.outside.parents.clone();
    {
        let (name, treasury, exec) = world.comp::<Corp>(c).map(|x| (x.name.clone(), x.treasury, x.exec))?;
        let persona = exec
            .and_then(|e| world.comp::<Personality>(e))
            .map(|p| Persona { greed: p.greed, pride: p.pride, lawfulness: p.lawfulness })
            .unwrap_or_default();
        let id = world.outside.next_id.max(WORLD_ACCOUNT + 1);
        world.outside.next_id = id + 1;
        let moved_target = (f64::from(cfg.found_parent_frac) * treasury as f64).round() as i64;
        world.outside.factions.push(OutsideFaction {
            id,
            name: format!("{name} Holdings"),
            kind: OutsideKind::Megacorp,
            treasury: 0,
            treasury_ref: moved_target.max(1),
            base_income: (f64::from(cfg.parent_income_rate) * moved_target as f64).round() as i64,
            base_upkeep: (f64::from(cfg.parent_upkeep_rate) * moved_target as f64).round() as i64,
            income_today: 0,
            market: 1.0,
            dead: false,
            books: Default::default(),
            ambient: None,
            parent: Some(Parent {
                persona,
                exposure: 0.5,
                rep: 0.5,
                health: 1.0,
                branches: vec![c],
                branch_name: name.clone(),
                edge: 1.0,
                ..Default::default()
            }),
        });
        let moved = ownership::cross_out(world, Some(c), id, moved_target, Flow::Remit, true);
        if let Some(x) = world.comp_mut::<Corp>(c) {
            x.parent = Some(id);
            x.cashflow_today += moved;
        }
        world.outside.found_streak.remove(&c);
        world.outside.today.parents_founded += 1;
        world.push_event(
            EventKind::ParentFounded,
            &[c],
            format!("{name} moved {moved} abroad and founded {name} Holdings"),
        );
        Some(id)
    }
}

/// God `FoundParent`: `corp` founds its parent now, whatever its treasury
/// and share (not past `max_accounts`, not a branch already).
pub fn god_found_parent(world: &mut World, corp: EntityId) -> Result<String, String> {
    if !parents_on(world) {
        return Err("FoundParent: the parents are off".into());
    }
    let x = world.comp::<Corp>(corp).ok_or("FoundParent: no corp".to_string())?;
    if x.parent.is_some() {
        return Err(format!("FoundParent: {} has a parent", x.name));
    }
    // M17 review: a parent founded on nothing would starve (its income and
    // upkeep scale with the coins it moves abroad): the natural threshold.
    let need = world.config.outside.parents.found_parent_treasury;
    if x.treasury < need {
        return Err(format!("FoundParent: {} holds {} (needs {need})", x.name, x.treasury));
    }
    if world.outside.factions.len() >= world.config.outside.parents.max_accounts {
        return Err("FoundParent: no account left".into());
    }
    let id = found_parent_for(world, corp).ok_or("FoundParent: no corp".to_string())?;
    let name = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    Ok(format!("God: {name} founded"))
}

/// § 2 Abandon: every branch cut loose, the Outsiders' tours ended, no
/// branch refounded for `abandon_days`.
pub fn abandon(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    let now = world.tick;
    let Some(p) = parent(world, id) else { return };
    let branches = p.branches.clone();
    for &b in &branches {
        if let Some(c) = world.comp_mut::<Corp>(b) {
            c.parent = None;
        }
    }
    for o in world.outside.outsiders.values_mut().filter(|o| o.from == id) {
        o.tour_until = o.tour_until.min(now);
        o.brother = false;
    }
    if let Some(p) = parent_mut(world, id) {
        p.branches.clear();
        p.held.clear();
        p.lost.clear();
        p.reinforce_left = 0;
        p.order = OutsideOrder::Abandon;
        p.order_since = now;
        p.abandoned_until = Some(now + u64::from(cfg.abandon_days) * TICKS_PER_DAY);
    }
    world.outside.today.abandoned += 1;
    let name = world.outside.faction(id).map_or_else(String::new, |f| f.name.clone());
    world.push_event(EventKind::Abandoned, &branches, format!("{name} cut its branches in the city loose"));
    // Phase 4: its scrip redeemed at health, or wiped below `scrip_par_min`.
    let h = parent(world, id).map_or(0.0, |p| p.health);
    if h < cfg.scrip_par_min {
        wipe_scrip(world, id);
    } else {
        let (paid, _) = redeem_all(world, id, h);
        world.outside.today.scrip_spent += paid;
    }
}
