use super::*;

/// § 3 death: `dead`, `ParentDied`; every corp branch bankrupt (M11's
/// estate rule), its gangs on their own, its Outsiders' tours ended, its
/// scrip wiped, its uplink dropped (a relink), its treasury absorbed abroad.
pub fn kill_parent(world: &mut World, id: OutsideId, why: &str) -> Result<String, String> {
    let now = world.tick;
    let Some(f) = world.outside.faction(id).filter(|f| f.parent.is_some() && !f.dead) else {
        return Err(format!("KillParent: no living parent {id}"));
    };
    if f.kind == OutsideKind::State {
        return Err("KillParent: the State never dies".to_string());
    }
    let name = f.name.clone();
    let branches = f.parent.as_ref().map(|p| p.branches.clone()).unwrap_or_default();
    world.push_event(EventKind::ParentDied, &branches, format!("{name} died abroad ({why})"));
    for &b in &branches {
        if world.has::<Corp>(b) {
            crate::systems::corps::bankrupt(world, b);
        }
    }
    for o in world.outside.outsiders.values_mut().filter(|o| o.from == id) {
        o.tour_until = o.tour_until.min(now);
        o.brother = false;
    }
    wipe_scrip(world, id);
    if let Some(f) = world.outside.faction_mut(id) {
        f.dead = true;
        let t = std::mem::take(&mut f.treasury);
        if let Some(p) = f.parent.as_mut() {
            p.branches.clear();
            p.reinforce_left = 0;
        }
        world.outside.minted -= t;
    }
    world.outside.today.parents_dead += 1;
    crate::systems::virt::mark_dirty(world);
    Ok(format!("{name} is dead"))
}

/// § 3: Data shipped to a branch whose tech upkeep would lapse for want of
/// it: `ship_data[tier]` units of the track into its first Lab.
pub fn ship_data(world: &mut World, id: OutsideId) {
    let cfg = world.config.outside.parents.clone();
    for c in corp_branches(world, id) {
        let Some(lab) = crate::systems::tech::labs_of(world, c).first().copied() else { continue };
        let Some(n) = crate::systems::virt::node_of_building(world, lab) else { continue };
        for t in crate::virt::Track::ALL {
            let (tier, lapse) = world.comp::<Corp>(c).map_or((1, 0), |x| (x.tech.tier_of(t), x.tech.lapse[t.index()]));
            if tier < 2 {
                continue;
            }
            let need = world.config.tech.upkeep_data_at(tier);
            if lapse == 0 && crate::systems::virt::holding(world, c, t) >= need {
                continue;
            }
            let want = cfg.ship_data.get(usize::from(tier) - 1).copied().unwrap_or(0);
            let units = parent(world, id).map_or(0, |p| p.data.get(t)).min(want);
            if units == 0 {
                continue;
            }
            if let Some(p) = parent_mut(world, id) {
                p.data.units[t.index()] -= units;
            }
            if let Some(node) = world.virt.node_mut(n) {
                node.store.units[t.index()] += units;
            }
            world.outside.today.data_shipped += 1;
            let (pname, lname) =
                (world.outside.faction(id).map_or_else(String::new, |f| f.name.clone()), world.name_of(lab));
            world.push_event(EventKind::DataShipped, &[c, lab], format!("{pname} shipped {units} {t} Data to {lname}"));
        }
    }
    sync_uplink(world, id);
}

/// A rival parent: another living megacorp parent whose corp branches
/// share a niche with this one's, the one its branches regard least (M15),
/// and that regard's value.
pub(super) fn rival_of(world: &World, id: OutsideId) -> Option<(OutsideId, f32)> {
    let mine = corp_branches(world, id);
    let niches: std::collections::BTreeSet<crate::components::Niche> =
        mine.iter().filter_map(|&c| world.comp::<Corp>(c)).flat_map(|c| c.niches.iter().copied()).collect();
    world
        .outside
        .parents()
        .filter(|f| f.id != id && f.kind == OutsideKind::Megacorp && !f.dead)
        .filter_map(|f| {
            let theirs: Vec<EntityId> = corp_branches(world, f.id)
                .into_iter()
                .filter(|&c| world.comp::<Corp>(c).is_some_and(|x| x.niches.iter().any(|n| niches.contains(n))))
                .collect();
            let v = mine
                .iter()
                .flat_map(|&a| theirs.iter().map(move |&b| (a, b)))
                .map(|(a, b)| crate::systems::reputation::regard(world, a, b).value)
                .fold(None, |m: Option<f32>, v| Some(m.map_or(v, |x| x.min(v))))?;
            Some((f.id, v))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
}

/// § 3 Campaign's action, weekly from its start: a run on the rival's
/// uplink when a branch Lab has a tier-3 fleet deck and a Researcher;
/// else, when the parent is lawless (< 0.4), a Beat on the rival branch's
/// exec. `CampaignPosted` on an action.
pub fn campaign_step(world: &mut World, id: OutsideId) {
    let now = crate::systems::midnight::stamp(world);
    let Some(p) = parent(world, id) else { return };
    if now.saturating_sub(p.order_since) % (7 * TICKS_PER_DAY) >= TICKS_PER_DAY {
        return;
    }
    let Some(rival) = p.rival else { return };
    let law = p.persona.lawfulness;
    let Some(target) = uplink_node(world, rival) else { return };
    let mut acted = None;
    for c in corp_branches(world, id) {
        let Some(run) = crate::systems::virt::corp_run(world, c) else { continue };
        let tier = run.deck.and_then(|d| world.comp::<crate::components::Asset>(d)).map_or(0, |a| a.tier);
        if tier < world.config.outside.parents.uplink_link_tier || world.run_orders.contains_key(&run.runner) {
            continue;
        }
        let order = crate::virt::RunOrder {
            patron: Some(c),
            purpose: crate::virt::Purpose::Data { wipe: law < 0.4 },
            target,
            chair: run.lab,
            not_before: now,
            expires: now + TICKS_PER_DAY,
            why: crate::virt::RunWhy::CorpOrder,
            mode: crate::virt::RunMode::Loud,
        };
        crate::systems::virt::give_order(world, run.runner, order);
        acted = Some(format!("a run on {}", crate::systems::virt::node_label(world, target)));
        break;
    }
    if acted.is_none() && law < 0.4 {
        let branch = corp_branches(world, id).into_iter().next();
        let exec = corp_branches(world, rival).into_iter().find_map(|c| world.comp::<Corp>(c).and_then(|x| x.exec));
        if let (Some(b), Some(e)) = (branch, exec.filter(|&e| crate::systems::law::living(world, e))) {
            let t = crate::contract::Target::Agent(e);
            let door = world.comp::<crate::components::Position>(e).map(|x| x.tile).unwrap_or_default();
            let broker = crate::systems::contracts::nearest_fixer_to(world, door);
            let q = crate::systems::contracts::quote(world, crate::contract::ContractKind::Beat, &t, broker);
            let sent = fund(world, id, b, q);
            let bexec = world.comp::<Corp>(b).and_then(|x| x.exec);
            let r = crate::systems::contracts::brain_post_with(
                world,
                Some(b),
                bexec,
                crate::contract::ContractKind::Beat,
                t,
                broker,
                crate::contract::Origin::Parent,
                Some(q),
                crate::contract::PostExtra::default(),
            );
            if r.is_ok() {
                acted = Some(format!("a Beat on {}", world.name_of(e)));
            } else {
                unfund(world, id, b, sent);
            }
        }
    }
    // Phase 5: no run and no Beat: a job abroad on the rival's depot by
    // the parent's own Outsiders.
    if acted.is_none() && crate::systems::abroad::campaign_post(world, id, rival) {
        acted = Some("a job on its depot abroad".to_string());
    }
    if let Some(what) = acted {
        world.outside.today.campaigns += 1;
        let (a, b) = (
            world.outside.faction(id).map_or_else(String::new, |f| f.name.clone()),
            world.outside.faction(rival).map_or_else(String::new, |f| f.name.clone()),
        );
        world.push_event(EventKind::CampaignPosted, &[], format!("{a}'s campaign against {b}: {what}"));
    }
}

/// A save from before phase 4: the parents' edge, Data and tech ceiling
/// from their rows (by name).
pub fn migrate_v24(world: &mut World) {
    if !parents_on(world) {
        return;
    }
    let rows = world.config.outside.parents.factions.clone();
    for f in world.outside.factions.iter_mut() {
        let Some(p) = f.parent.as_mut() else { continue };
        if p.edge > 0.0 {
            continue;
        }
        p.edge = 1.0;
        if let Some(r) = rows.iter().find(|r| r.name == f.name) {
            p.data = crate::virt::DataStore { units: r.data };
            p.tech = r.tech;
        }
    }
    crate::systems::virt::mark_dirty(world);
}
