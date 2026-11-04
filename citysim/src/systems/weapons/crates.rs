use super::*;

/// At `crate_hour`: a gang with members, short of its armoury target plus
/// its dealer's float (`deal_float`), with no crate within `crate_days` and
/// a standing Freight Yard, ships a crate of up to `crate_size` at
/// `crate_unit` each: a syndicate's branch's crate is the parent's, paid
/// from its treasury abroad (in kind, `ParentFund`'s shape); a gang on its
/// own pays the World from the coins above its payroll
/// (`crate_reserve_days` of stipends for the roster and one recruit). A Loader on shift at
/// the Yard (the lowest id) takes the bribe; customs seizes on `p_seize`
/// (× `bribe_mult` bribed) into the Precinct, filing `Smuggling` on the
/// leader; else the crate lands in the armoury.
pub fn crates(world: &mut World) {
    let cfg = world.config.weapons.clone();
    let now = world.tick;
    let day = world.day();
    for gang in world.gangs() {
        let Some(h) = world.hideout_of(gang) else { continue };
        let members = world.comp::<Gang>(gang).map_or(0, |g| g.members.len());
        if members == 0 {
            continue;
        }
        let want = (armoury_target(world, gang) + cfg.deal_float as usize).saturating_sub(stock_at(world, h).len());
        if want == 0 {
            continue;
        }
        if world.jobs2.arms.last_crate.get(&gang).is_some_and(|&t| now < t + u64::from(cfg.crate_days) * TICKS_PER_DAY)
        {
            continue;
        }
        let room = cap(world).saturating_sub(alive(world));
        // A syndicate's branch: the parent ships the crate in kind (its
        // treasury abroad pays, `ParentFund`'s shape); a gang on its own
        // pays from the coins above `crate_reserve_days` of stipends for the
        // roster and one recruit (the payroll first).
        let unit = cfg.crate_unit.max(1);
        let parent = crate::systems::parents::syndicate_id(world, gang);
        let budget = match parent {
            Some(id) => crate::systems::parents::treasury_abroad(world, id),
            None => {
                let reserve = world.config.social.gang_stipend.max(0)
                    * (members as i64 + 1)
                    * i64::from(cfg.crate_reserve_days.max(1));
                world.purse(Some(gang)) - reserve
            }
        };
        let afford = ((budget - cfg.bribe).max(0) / unit) as usize;
        let n = want.min(cfg.crate_size as usize).min(afford).min(room);
        if n == 0 || world.purse(Some(gang)) < cfg.bribe {
            continue;
        }
        let price = unit * n as i64;
        let from = world.comp::<Building>(h).map(|b| b.door).unwrap_or_default();
        let yard = world
            .buildings_of_kind(BuildingKind::FreightYard)
            .iter()
            .copied()
            .filter(|&y| crate::systems::industry::open(world, y))
            .filter_map(|y| world.comp::<Building>(y).map(|b| (b.door.manhattan(from), y)))
            .min()
            .map(|(_, y)| y);
        let Some(yard) = yard else { continue };
        let paid = match parent {
            Some(id) => {
                if crate::systems::parents::spend_abroad(world, id, price) {
                    price
                } else {
                    0
                }
            }
            None => ownership::cross_out(world, Some(gang), crate::outside::WORLD_ACCOUNT, price, Flow::Import, false),
        };
        if paid < price {
            continue;
        }
        world.jobs2.arms.last_crate.insert(gang, now);
        let tod = world.tick_of_day();
        let loader = world.staff_of(yard).iter().copied().find(|&s| {
            world.comp::<Job>(s).is_some_and(|j| {
                j.on_shift(tod)
                    && crate::systems::trades::row_of(world, j.role).is_some_and(|r| r.model == TradeModel::Handling)
            }) && !world.has::<Sentence>(s)
        });
        let mut p = cfg.p_seize;
        if let Some(l) = loader {
            if ownership::pay(world, Some(gang), Some(l), cfg.bribe, Flow::Bribe) > 0 {
                p *= cfg.bribe_mult;
            }
        }
        let gname = world.owner_label(Some(gang));
        let yname = world.name_of(yard);
        if hash(world, PURPOSE_CRATE, key(gang), day) < p {
            let Some(pre) = precinct(world) else { continue };
            for _ in 0..n {
                let pv = prov(world, ProvKind::Seized, Some(pre));
                spawn_weapon(world, cfg.crate_tier, None, AssetLoc::Stock(pre), pv, false);
            }
            ownership::jobs2_row(world).crates_seized += 1;
            let leader = world.comp::<Gang>(gang).and_then(|g| g.leader.or_else(|| g.members.first().copied()));
            if let Some(l) = leader {
                crate::systems::law::file_report(world, crate::components::Crime::Smuggling, l, None);
            }
            world.push_event(
                EventKind::CrateSeized,
                &[gang, yard],
                format!("customs seized {gname}'s crate at {yname}"),
            );
        } else {
            let mut landed = 0;
            for _ in 0..n {
                let pv = prov(world, ProvKind::Smuggled, Some(gang));
                if spawn_weapon(world, cfg.crate_tier, Some(gang), AssetLoc::Stock(h), pv, false).is_some() {
                    landed += 1;
                }
            }
            ownership::jobs2_row(world).crates_landed += 1;
            world.push_event(
                EventKind::CrateLanded,
                &[gang, yard],
                format!("a crate of {landed} weapons for {gname} passed {yname}"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Petty theft and the fence (Dylan's answer 1)
// ---------------------------------------------------------------------------

/// Room for one more petty item theft today (R3's cap).
fn petty_room(world: &World) -> bool {
    world.jobs2.arms.petty_today < per_capita_day(world, world.config.weapons.petty_per_1000_day)
}

/// `exec::steal_food` at a Market: on `p_pickpocket` the thief also lifts
/// the best item a body within `petty_reach` carries (a concealment
/// contest: the victim's sight against the thief's stealth); with no
/// carrier near, the till of a stall in the Market's district (up to
/// `stall_take_max` of its owner's coins). A Theft each, the victim's.
pub fn market_petty(world: &mut World, thief: EntityId) {
    if !on(world) || !petty_room(world) {
        return;
    }
    let cfg = world.config.weapons.clone();
    let Some(tile) = world.comp::<Position>(thief).map(|p| p.tile) else { return };
    let tick = world.tick;
    if hash(world, PURPOSE_PETTY, key(thief), tick) >= cfg.p_pickpocket {
        return;
    }
    let near = world.agents_near(tile, cfg.petty_reach, None);
    // The pressure pass S6 (Dylan 2026-10-27): nobody under 18 is a victim.
    let victim = near.into_iter().filter(|&o| o != thief && !crate::systems::ages::is_protected(world, o)).find(|&o| {
        world.has::<Identity>(o) && !world.has::<Corpse>(o) && world.comp::<Kit>(o).is_some_and(|k| k.weapon > 0)
    });
    if let Some(v) = victim {
        let stealth = crate::systems::law::stealth(world, thief);
        let step = cfg.conceal_step;
        let p_lift = crate::systems::security::contest_p(
            f32::from(crate::systems::security::thief_tier(stealth)),
            f32::from(sight_tier(world, v)),
            step,
        );
        if hash(world, PURPOSE_PETTY, key(v), tick) >= p_lift {
            return;
        }
        let Some(a) = best_of(world, &carried_by(world, v)) else { return };
        let p = prov(world, ProvKind::Stolen, Some(thief));
        hand_over(world, a, Some(thief), AssetLoc::Carried(thief), p, true);
        world.jobs2.arms.petty_today += 1;
        ownership::jobs2_row(world).pickpockets += 1;
        let text = format!("{} lifted a Weapon from {}", world.name_of(thief), world.name_of(v));
        world.push_event(EventKind::Theft, &[thief, v], text);
        crate::systems::law::raise_crime(world, thief, Some(v), crate::components::Crime::Theft, tile);
        return;
    }
    // A stall of the Market's district with a take today (the till on the
    // thief's way out). Round 6 V3 (`OPEN-238`): the nearest stall to the
    // thief (was the lowest owner id), never a minor's, and not one emptied
    // in the last `stall_rob_gap_hours` (its owner minds the till after):
    // V14 `vendor_9648 d23` took ten Market thieves from across the Sump in
    // six hours as the district's lowest owner id.
    let d = world.district_of(tile);
    let gap = u64::from(cfg.stall_rob_gap_hours) * crate::time::TICKS_PER_HOUR;
    let stall = world
        .jobs2
        .pitches
        .iter()
        // Jobs & lives phase 4: a street worker's pitch is no stall.
        .filter(|(&o, p)| {
            o != thief && p.take_today > 0 && p.district == d && !crate::systems::vice::is_street_pitch(world, p)
        })
        .filter(|(&o, _)| world.jobs2.arms.till_robbed.get(&o).is_none_or(|&t| tick >= t + gap))
        .filter(|(&o, _)| !crate::systems::ages::is_protected(world, o))
        .map(|(&o, p)| (crate::systems::law::chebyshev(p.spot, tile), o, p.take_today))
        .min()
        .map(|(_, o, take)| (o, take));
    let Some((owner, take)) = stall else { return };
    let coins = take.min(cfg.stall_take_max).min(world.comp::<Wallet>(owner).map_or(0, |w| w.coins.max(0)));
    if coins <= 0 {
        return;
    }
    if let Some(w) = world.comp_mut::<Wallet>(owner) {
        w.coins -= coins;
    }
    if let Some(w) = world.comp_mut::<Wallet>(thief) {
        w.coins += coins;
    }
    if let Some(p) = world.jobs2.pitches.get_mut(&owner) {
        p.take_today -= coins;
    }
    world.jobs2.arms.petty_today += 1;
    world.jobs2.arms.till_robbed.insert(owner, tick);
    ownership::jobs2_row(world).stall_thefts += 1;
    let text = format!("{} emptied the till of {}'s stall ({coins} coins)", world.name_of(thief), world.name_of(owner));
    world.push_event(EventKind::Theft, &[thief, owner], text);
    crate::systems::law::raise_crime(world, thief, Some(owner), crate::components::Crime::Theft, tile);
}

/// `exec::steal_food` at a Home: the thief also takes the best item kept
/// there (stolen; the Theft is the food's).
pub fn home_petty(world: &mut World, thief: EntityId, home: EntityId) {
    if !on(world) || !petty_room(world) {
        return;
    }
    let Some(a) = best_of(world, &kept_at(world, home)) else { return };
    let p = prov(world, ProvKind::Stolen, Some(thief));
    hand_over(world, a, Some(thief), AssetLoc::Carried(thief), p, true);
    world.jobs2.arms.petty_today += 1;
    let clothing = world.comp::<Asset>(a).is_some_and(|x| x.kind == AssetKind::Clothing);
    let row = ownership::jobs2_row(world);
    row.home_thefts += 1;
    if clothing {
        row.clothes_stolen += 1;
    }
    let what = if clothing { "Clothing" } else { "Weapon" };
    let text = format!("{} took a {what} from {}", world.name_of(thief), world.name_of(home));
    world.push_event(EventKind::Theft, &[thief, home], text);
}

/// The gang fence for items: the stolen items `thief` carries, sold to
/// `gang` at `fence_frac` of their value (`Flow::Fence`, while the gang
/// can pay) into its armoury. Returns the items fenced.
pub fn fence_items(world: &mut World, thief: EntityId, gang: EntityId) -> u32 {
    if !on(world) {
        return 0;
    }
    let Some(h) = world.hideout_of(gang) else { return 0 };
    let frac = world.config.weapons.fence_frac;
    let mut n = 0;
    // Jobs & lives phase 4: the stolen clothing carried too.
    let items: Vec<EntityId> = assets::assets_at(world, thief)
        .iter()
        .copied()
        .filter(|&a| world.comp::<Asset>(a).is_some_and(|x| x.kind.is_item() && x.loc == AssetLoc::Carried(thief)))
        .collect();
    for a in items {
        let Some((stolen, value, owner)) = world.comp::<Asset>(a).map(|x| (x.stolen, x.value, x.owner)) else {
            continue;
        };
        if !stolen || owner != Some(thief) {
            continue;
        }
        let pay = ((value as f32 * frac).round() as i64).max(1);
        if world.purse(Some(gang)) < pay {
            break;
        }
        ownership::pay(world, Some(gang), Some(thief), pay, Flow::Fence);
        assets::into_gang_stock(world, a, gang, h);
        let pv = prov(world, ProvKind::Stolen, Some(thief));
        if let Some(x) = world.comp_mut::<Asset>(a) {
            x.prov = Some(pv);
        }
        ownership::jobs2_row(world).fenced += 1;
        n += 1;
    }
    n
}

/// Midnight: a thief still carrying stolen items fences them through his
/// gang, else the gang holding his district (a free agent keeps them).
pub(super) fn fence_daily(world: &mut World) {
    let thieves: Vec<EntityId> = {
        let mut v: Vec<EntityId> = world
            .jobs2
            .arms
            .index
            .iter()
            .chain(world.jobs2.fashion.index.iter())
            .filter_map(|&a| world.comp::<Asset>(a))
            .filter(|x| x.stolen)
            .filter_map(|x| match x.loc {
                AssetLoc::Carried(h) if x.owner == Some(h) => Some(h),
                _ => None,
            })
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    for t in thieves {
        if !free_adult(world, t) {
            continue;
        }
        let gang = world.gang_of(t).or_else(|| {
            let tile = world.comp::<Position>(t)?.tile;
            match world.district(world.district_of(tile)).control {
                crate::components::Controller::Gang(g) if world.has::<Gang>(g) => Some(g),
                _ => None,
            }
        });
        if let Some(g) = gang {
            fence_items(world, t, g);
        }
    }
}
