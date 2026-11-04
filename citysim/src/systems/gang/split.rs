use super::*;

/// D36: a decapitated gang (the old leader dead or jailed, a new one in)
/// may split. The lieutenant (runner-up of `leader_ranking`) founds a
/// splinter when the gang holds Homes in two districts or more, the
/// lieutenant's strength is at least `split_strength_ratio` of the new
/// leader's, the mean member loyalty is below `split_loyalty`, the city has
/// fewer than `max_gangs` gangs, and (with `roll`) a draw under
/// `split_base × (1 − mean loyalty)` lands. The splinter takes the held
/// district other than the leader's Home district with the most held Homes
/// (ties lower id); its Hideout is a gang-held squat there, else the vacant
/// Lot there nearest the district's centroid, else no split. It takes
/// every member whose Home (or squat) is there or who likes the lieutenant
/// better than the leader, those Homes' claims, and the treasury share by
/// headcount. `Split` event; Enemy edges across the rosters (30 pairs at
/// most); `Shock::Split` on both. Returns the splinter, or why not.
pub fn split(world: &mut World, gang: EntityId, old_leader: Option<EntityId>, roll: bool) -> Result<EntityId, String> {
    split_inner(world, gang, old_leader, roll, None)
}

/// M17 phase 4b: the loser of a civil war leads a splinter of its
/// `following` (D36's structure tests only: two held districts, the cap, a
/// Hideout site). The splinter takes the held district apart from the
/// leader's, its claims, and the treasury share by headcount.
pub fn split_led(
    world: &mut World,
    gang: EntityId,
    loser: EntityId,
    following: &[EntityId],
) -> Result<EntityId, String> {
    split_inner(world, gang, None, false, Some((loser, following)))
}

fn split_inner(
    world: &mut World,
    gang: EntityId,
    old_leader: Option<EntityId>,
    roll: bool,
    led: Option<(EntityId, &[EntityId])>,
) -> Result<EntityId, String> {
    use crate::systems::raid;
    let cfg = world.config.gangs.clone();
    if roll && cfg.split_base <= 0.0 {
        return Err("splits are off".into());
    }
    let (leader, lt) = match led {
        Some((loser, _)) => {
            let Some(leader) = world.comp::<Gang>(gang).and_then(|g| g.leader).filter(|&l| l != loser) else {
                return Err("no leader".into());
            };
            (leader, loser)
        }
        None => {
            let ranking = leader_ranking(world, gang);
            let (Some(&leader), Some(&lt)) = (ranking.first(), ranking.get(1)) else {
                return Err("no lieutenant".into());
            };
            (leader, lt)
        }
    };
    let held = held_districts(world, gang);
    if held.len() < 2 {
        return Err(format!("holds Homes in {} district(s)", held.len()));
    }
    // M12 phase 5: the god `SplitGang` (`roll` false) skips the character
    // tests too (the lieutenant's strength, the gang's loyalty); only the
    // structure (a lieutenant, two held districts, the cap, a Hideout site)
    // can refuse it.
    let (sl, sll) = (raid::strength(world, leader), raid::strength(world, lt));
    if roll && sll < cfg.split_strength_ratio * sl {
        return Err(format!("lieutenant too weak ({sll:.2} vs {sl:.2})"));
    }
    let members = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let loyalty = members.iter().map(|&m| world.comp::<Personality>(m).map_or(0.5, |p| p.loyalty)).sum::<f32>()
        / members.len().max(1) as f32;
    if roll && loyalty >= cfg.split_loyalty {
        return Err(format!("too loyal ({loyalty:.2})"));
    }
    // Fix pass (phase 4 review): an emptied gang (a ghost waiting to
    // re-form, or never to) does not count toward the cap.
    let live = world
        .gang_list()
        .iter()
        .filter(|&&g| world.comp::<Gang>(g).is_some_and(|x| !(x.emptied && x.members.is_empty())))
        .count();
    if live >= cfg.max_gangs {
        return Err(format!("{live} gangs already"));
    }
    if roll {
        let p = cfg.split_base * (1.0 - loyalty);
        let r: f32 = world.rng.world().random();
        if r >= p {
            return Err(format!("held together ({r:.2} vs {p:.2})"));
        }
    }
    let leader_d = home_or_squat(world, leader).map(|b| world.district_of_building(b));
    let Some(&(sd, _)) =
        held.iter().filter(|(d, _)| Some(*d) != leader_d).max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
    else {
        return Err("no district apart from the leader's".into());
    };
    let Some(hideout) = splinter_hideout(world, gang, sd) else {
        return Err(format!("no squat or Lot in {}", world.district_name(sd)));
    };
    // The splinter.
    let used: Vec<String> =
        world.gang_list().iter().filter_map(|&g| world.comp::<Gang>(g).map(|g| g.name.clone())).collect();
    let old_name = world.comp::<Gang>(gang).map_or_else(String::new, |g| g.name.clone());
    let name = cfg
        .splinter_names
        .iter()
        .find(|n| !used.contains(n))
        .cloned()
        .unwrap_or_else(|| format!("{old_name} Splinter {}", world.gang_list().len()));
    // Fix pass (phase 4 review): a jailed old boss stays with the gang it
    // ran (unless it is the lieutenant who leads the splinter).
    let boss = world.comp::<Gang>(gang).and_then(|g| g.boss);
    let movers: Vec<EntityId> = members
        .iter()
        .copied()
        .filter(|&m| m != leader)
        .filter(|&m| m == lt || Some(m) != boss)
        .filter(|&m| match led {
            // A war's loser takes its following, wherever they live.
            Some((_, following)) => m == lt || following.contains(&m),
            None => {
                m == lt
                    || home_or_squat(world, m).is_some_and(|b| world.district_of_building(b) == sd)
                    || world.edge(m, lt).map_or(0.0, |e| e.affinity) > world.edge(m, leader).map_or(0.0, |e| e.affinity)
            }
        })
        .collect();
    let treasury = world.comp::<Gang>(gang).map_or(0, |g| g.treasury.max(0));
    let share = treasury * movers.len() as i64 / members.len().max(1) as i64;
    let splinter = world.spawn();
    let mut sg = Gang::new(name.clone(), hideout, share);
    sg.empty_since = None;
    sg.split_from = Some(gang);
    // A war's loser founds and leads its splinter.
    if led.is_some() {
        sg.crowned = Some(lt);
    }
    world.insert(splinter, sg);
    // M14 V4: the splinter's Hideout (a converted derelict, or a Lot built
    // with no owner yet) gets its node at the next tick, not the next midnight.
    crate::systems::virt::mark_dirty(world);
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.treasury -= share;
        g.members.retain(|m| !movers.contains(m));
        if g.raid_target.is_some_and(|t| t == hideout) {
            g.raid_target = None;
        }
    }
    for &m in &movers {
        if let Some(gm) = world.comp_mut::<GangMember>(m) {
            gm.gang = splinter;
        }
    }
    // Claims on the splinter district's Homes go with it.
    let moved: Vec<EntityId> = world
        .comp::<Gang>(gang)
        .map(|g| g.territory.iter().copied().filter(|&h| world.district_of_building(h) == sd).collect())
        .unwrap_or_default();
    for &h in &moved {
        if let Some(b) = world.comp_mut::<Building>(h) {
            if let Some(c) = b.claim.as_mut() {
                c.gang = splinter;
            }
        }
    }
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.territory.retain(|h| !moved.contains(h));
    }
    if let Some(g) = world.comp_mut::<Gang>(splinter) {
        g.members = movers.clone();
        g.territory = moved.clone();
    }
    recompute_leader(world, splinter);
    recompute_leader(world, gang);
    // M15 W31: an unchromed, lawful lieutenant's splinter is Purist.
    crate::systems::creeds::splinter_creed(world, splinter, lt);
    // The halves are enemies: 30 pairs each way at most.
    let rest = world.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
    let mut pairs = 0;
    'outer: for &a in &movers {
        for &b in &rest {
            if pairs >= 30 {
                break 'outer;
            }
            social::make_enemy(world, a, b, -0.5);
            pairs += 1;
        }
    }
    push_shock(world, gang, Shock::Split);
    push_shock(world, splinter, Shock::Split);
    let ltn = world.name_of(lt);
    let dn = world.district_name(sd).to_string();
    let why = match (led, old_leader) {
        (Some(_), _) => format!(" after losing the war for {old_name}"),
        (None, Some(o)) => format!(" after {} fell", world.name_of(o)),
        (None, None) => String::new(),
    };
    world.push_event(
        EventKind::Split,
        &[gang, splinter, lt],
        format!(
            "{ltn} split from {old_name} with {} members and {} Blocks in {dn}{why}: the {name}",
            movers.len(),
            moved.len()
        ),
    );
    faction::rethink(world, splinter);
    faction::rethink(world, gang);
    Ok(splinter)
}

/// The Home an agent lives in, else the derelict it squats.
fn home_or_squat(world: &World, a: EntityId) -> Option<EntityId> {
    world
        .comp::<Household>(a)
        .and_then(|h| h.home)
        .or_else(|| world.comp::<crate::components::Squatter>(a).map(|s| s.building))
}

/// D36: the splinter's Hideout in district `d`: a derelict the gang holds
/// there (lowest id), turned into a Hideout; else the vacant Lot there
/// nearest the district's centroid, built as one. `None` with neither.
fn splinter_hideout(world: &mut World, gang: EntityId, d: DistrictId) -> Option<EntityId> {
    let squat = world.comp::<Gang>(gang).and_then(|g| {
        g.territory
            .iter()
            .copied()
            .find(|&b| world.district_of_building(b) == d && crate::systems::street::is_derelict(world, b))
    });
    if let Some(b) = squat {
        convert_to_hideout(world, gang, b);
        return Some(b);
    }
    let centroid = world.district(d).centroid;
    let lot = world
        .buildings_of_kind(BuildingKind::Lot)
        .iter()
        .copied()
        .filter_map(|l| world.comp::<Building>(l).filter(|b| !b.demolished).map(|b| (b.door, l)))
        .filter(|&(door, _)| world.district_of(door) == d)
        .map(|(door, l)| (door.manhattan(centroid), l))
        .min()
        .map(|(_, l)| l)?;
    crate::systems::founding::build_on_lot(world, lot, BuildingKind::Hideout, None).ok()
}

/// A held derelict becomes a Hideout: its squatters put out, its claim and
/// its place in the old gang's territory dropped, its kind and capacity
/// changed, the district lists rebuilt.
pub fn convert_to_hideout(world: &mut World, gang: EntityId, b: EntityId) {
    crate::systems::street::evict_squatters(world, b, "a gang's new Hideout");
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.territory.retain(|&h| h != b);
    }
    let cap = world.config.buildings.hideout.capacity;
    let interior = world
        .comp::<Building>(b)
        .map_or(0, |bd| usize::from(bd.rect.w.saturating_sub(2)) * usize::from(bd.rect.h.saturating_sub(2)));
    let old_kind = world.comp::<Building>(b).map(|bd| bd.kind);
    if let Some(bd) = world.comp_mut::<Building>(b) {
        bd.kind = BuildingKind::Hideout;
        bd.derelict = false;
        bd.full_capacity = None;
        bd.empty_since = None;
        bd.claim = None;
        bd.rent_per_day = 0;
        bd.stock_food = 0;
        bd.capacity = u16::try_from(interior).unwrap_or(u16::MAX).min(cap);
    }
    if let Some(k) = old_kind {
        if let Some(v) = world.buildings_by_kind_mut().get_mut(&k) {
            v.retain(|&x| x != b);
        }
    }
    let list = world.buildings_by_kind_mut().entry(BuildingKind::Hideout).or_default();
    if let Err(i) = list.binary_search(&b) {
        list.insert(i, b);
    }
    crate::systems::districts::rebuild(world);
    // M14 V4: a kind change outside `transfer_building` relinks the plane.
    crate::systems::virt::mark_dirty(world);
}
