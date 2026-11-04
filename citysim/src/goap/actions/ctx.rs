use super::*;

impl PlanCtx {
    /// Full context for planning (distances and the guard scan included).
    pub fn build(world: &World, agent: EntityId, target: Option<EntityId>) -> PlanCtx {
        PlanCtx::build_inner(world, agent, target, false, None)
    }

    /// [`PlanCtx::build`] for planning `goal`: the inputs only other goals'
    /// actions read are skipped, and `allowed` offers each goal its own.
    pub fn build_for(world: &World, agent: EntityId, target: Option<EntityId>, goal: GoalKind) -> PlanCtx {
        PlanCtx::build_inner(world, agent, target, false, Some(goal))
    }

    /// Context for re-checking one action's preconditions at step start:
    /// skips the O(population) guard scan and the distance table, which
    /// only costs and GoTo need.
    pub fn build_light(world: &World, agent: EntityId, target: Option<EntityId>) -> PlanCtx {
        PlanCtx::build_inner(world, agent, target, true, None)
    }

    fn build_inner(
        world: &World,
        agent: EntityId,
        target: Option<EntityId>,
        light: bool,
        goal: Option<GoalKind>,
    ) -> PlanCtx {
        // M13 review: is an input read by `goals`' actions (all with no goal)?
        let wants = |goals: &[GoalKind]| goal.is_none_or(|g| goals.contains(&g));
        let p = world.comp::<Personality>(agent);
        let s = world.comp::<Skills>(agent);
        let job = world.comp::<Job>(agent);
        let hunger = world.comp::<crate::components::Needs>(agent).map_or(1.0, |n| n.hunger);
        let pos = world.comp::<Position>(agent);
        let tile = pos.map(|p| p.tile);
        let home = world.comp::<Household>(agent).and_then(|h| h.home);
        let sight = world.config.crime.sight_day_crime; // "guard8"

        // The gang this agent would join (`gang::eligible`, and the Hideout a
        // non-member means): read once (Scale S4 perf: it walks the edges).
        let recruit = crate::systems::gang::recruit_gang(world, agent);
        let guard8 = !light
            && tile.is_some_and(|t| {
                world
                    .guards()
                    .iter()
                    .any(|&g| g != agent && world.comp::<Position>(g).is_some_and(|gp| gp.tile.manhattan(t) <= sight))
            });
        // M10: the agent's own Market (the one it is in, else the nearest).
        let market = world.local(agent, BuildingKind::Market);
        let stock_of = |kind: BuildingKind| -> u32 {
            world.local(agent, kind).and_then(|b| world.comp::<Building>(b)).map_or(0, |b| b.stock_food)
        };
        let farm = job
            .and_then(|j| j.employer)
            .filter(|&e| world.comp::<Building>(e).is_some_and(|b| b.kind == BuildingKind::Farm));
        let tod = world.tick_of_day();
        let day = world.day();
        let wage_desk = job.and_then(|_| world.wage_desk(agent));
        let wage_at = match wage_desk.and_then(|d| world.comp::<Building>(d)) {
            Some(b) if b.kind != BuildingKind::Hall => LocationKey::of_building(b.kind),
            _ => LocationKey::Hall,
        };
        let payer = job.and_then(|j| j.employer).and_then(|e| world.owner_of(e));

        // Distances: every singleton building, the agent's Home, the employer's
        // workplace for Farm, and the bound target.
        let origin = pos.map(|p| match p.building.and_then(|b| world.comp::<Building>(b)) {
            Some(b) => world.outside_door(b),
            None => p.tile,
        });
        let mut dist = BTreeMap::new();
        if let (Some(o), false) = (origin, light) {
            dist.insert(LocationKey::Street, 0);
            if let Some(&(t, _)) = target.and_then(|s| world.last_seen().get(&s)) {
                dist.insert(LocationKey::SuspectTile, o.manhattan(t));
            }
            let mut add = |key: LocationKey, b: Option<EntityId>| {
                if let Some(door) = b.and_then(|b| world.comp::<Building>(b)).map(|b| b.door) {
                    dist.insert(key, o.manhattan(door));
                }
            };
            add(LocationKey::Home, home);
            add(LocationKey::Farm, farm.or_else(|| world.local(agent, BuildingKind::Farm)));
            for kind in [
                BuildingKind::Market,
                BuildingKind::Bar,
                BuildingKind::Jail,
                BuildingKind::Cemetery,
                BuildingKind::Hall,
                BuildingKind::Warehouse,
            ] {
                add(LocationKey::of_building(kind), world.local(agent, kind));
            }
            add(LocationKey::Hideout, crate::systems::gang::hideout_with(world, agent, || recruit));
            if wage_at == LocationKey::Workplace {
                add(LocationKey::Workplace, world.wage_desk(agent));
            } else if job.and_then(|j| j.employer).is_some_and(|e| {
                crate::systems::jobs::is_l2_building(world, e)
                    // L2 shadow fixes item 11: a city-owned Feed's or Lab's
                    // staff work at it too (the Civic Wire's reporter was
                    // sent to the Hall: Work "Unreachable" all week).
                    || world.comp::<Building>(e).is_some_and(|b| LocationKey::of_building(b.kind) == LocationKey::Workplace)
            }) {
                // L2: a city-owned venue's or Fab's staff work at it (the
                // wage desk is the Hall).
                add(LocationKey::Workplace, job.and_then(|j| j.employer));
            }
            // Jobs and room J13: a trade's staff work at their employer
            // (a Market's, a Hotel's, a Block's), whoever pays.
            if job.is_some_and(|j| j.role.is_trade()) {
                add(LocationKey::Workplace, job.and_then(|j| j.employer));
            }

            // M13 D47: a Mechanic's Garage (its employer).
            if job.is_some_and(|j| j.role == Role::Mechanic) {
                add(LocationKey::Garage, world.resolve_building(agent, LocationKey::Garage, None));
            }
            add(LocationKey::TargetHome, target);
            if let Some(b) = world
                .comp::<crate::components::Brain>(agent)
                .and_then(|b| b.patrol_route.get(usize::from(b.patrol_legs) % b.patrol_route.len().max(1)).copied())
            {
                add(LocationKey::PatrolWaypoint, Some(b));
            }
            // A corpse is reached at its tile, or through its building's door.
            if let Some(c) = target.filter(|&t| world.has::<crate::components::Corpse>(t)) {
                if let Some(p) = world.comp::<Position>(c) {
                    let there = p.building.and_then(|b| world.comp::<Building>(b)).map_or(p.tile, |b| b.door);
                    dist.insert(LocationKey::CorpseTile, o.manhattan(there));
                }
            }
            if let Some(t) = crate::systems::raid::target_tile(world, agent) {
                dist.insert(LocationKey::RaidTarget, o.manhattan(t));
            }
            // M12 D37: the muster point, while an expedition is pending.
            if crate::systems::raid::raid_pending(world, agent) {
                let there = match crate::systems::raid::muster_point(world, agent) {
                    Some(crate::systems::raid::MusterAt::Inside(b)) => world.comp::<Building>(b).map(|bd| bd.door),
                    Some(crate::systems::raid::MusterAt::Door(t)) => Some(t),
                    None => None,
                };
                if let Some(t) = there {
                    dist.insert(LocationKey::MusterPoint, o.manhattan(t));
                }
            }
            // L2 L8: a sweeper's beat (the dirtiest street tile of it).
            if job.is_some_and(|j| j.role == Role::Sanitation) && crate::systems::jobs::sweep_on(world) {
                if let Some(t) = crate::systems::jobs::beat_tile(world, agent) {
                    dist.insert(LocationKey::Beat, o.manhattan(t));
                }
            }
        }

        // M13 D33/D36: the target is a living agent this agent hunts: its
        // episode's quarry, or its gang's Harvest target.
        let episode = crate::systems::chrome::in_episode(world, agent);
        let victim_target = wants(&[GoalKind::GangWork])
            && target.is_some_and(|t| {
                world.gang_of(agent).is_some_and(|g| crate::systems::chrome::gang_harvest_target(world, g) == Some(t))
            });
        let victim_plan =
            target.is_some_and(|t| t != agent && crate::systems::law::living(world, t)) && (episode || victim_target);
        // M13 D34/D35: the Clinic and the body.
        let target_clinic =
            target.filter(|&t| world.comp::<Building>(t).is_some_and(|b| b.kind == BuildingKind::Clinic));
        let looting = wants(&[GoalKind::Loot, GoalKind::GangWork]);
        let may_rip = looting && crate::systems::chrome::may_rip(world, agent);
        let loot_target = looting && target.is_some_and(|t| crate::systems::chrome::may_loot(world, agent, t));
        let loot_implants = may_rip
            && target.is_some_and(|t| {
                world.comp::<crate::components::Corpse>(t).is_some_and(|c| !c.buried)
                    && crate::systems::assets::assets_at(world, t).iter().any(|&a| {
                        world
                            .comp::<crate::components::Asset>(a)
                            .is_some_and(|x| x.kind.is_implant() && x.loc == crate::components::AssetLoc::Installed(t))
                    })
            });
        // M13 D38-D40: dealing, buying and Detox.
        let stims_on = world.config.assets.enabled;
        // M14: every Virt input reads false with the plane off.
        let gang_hideout = world.gang_of(agent).and_then(|g| world.hideout_of(g));
        let dealer = stims_on
            && wants(&[GoalKind::GangWork])
            && target.is_some()
            && crate::systems::stims::dealer_target(world, agent) == target;
        let source_price = if stims_on && wants(&[GoalKind::GetHigh]) {
            target.and_then(|t| crate::systems::stims::source_price(world, t))
        } else {
            None
        };
        let stim_source = source_price.is_some();
        let coins_now = world.comp::<Wallet>(agent).map_or(0, |w| w.coins);
        let treating = wants(&[GoalKind::Treat]);
        let detox_pick =
            stims_on && treating && target_clinic.is_some() && crate::systems::stims::detox_drives(world, agent);
        // M13 D26: the thief's vehicles (the cheap tests first: a vehicle
        // bound, or one held).
        let target_vehicle =
            target.is_some_and(|t| world.comp::<crate::components::Asset>(t).is_some_and(|x| x.kind.is_vehicle()));
        let held_vehicle = stims_on && crate::systems::vehicles::stolen_held_by(world, agent).is_some();
        let withdrawal_bonus = if stims_on && crate::systems::stims::in_withdrawal(world, agent) {
            world.config.stims.withdrawal_steal_bonus
        } else {
            0.0
        };
        let steal_can_buy = if crate::systems::fixes::buy_not_steal(world)
            && coins_now >= crate::systems::life::meal_price(world, agent)
        {
            world.config.life.steal_can_buy
        } else {
            0.0
        };
        // M12 D21/D27: the street's rungs (homeless agents only; cheap scans
        // over the few Hotels and derelicts).
        let homeless = home.is_none();
        // L1: a bed nearer than Home (planning only: the step re-check
        // reads the symbols, and a walk under way keeps its target).
        let sleeping = !light && wants(&[GoalKind::Sleep]);
        let away_hotel = if sleeping && !homeless { crate::systems::life::away_hotel(world, agent) } else { None };
        let hideout_bed = sleeping && crate::systems::life::hideout_bed(world, agent).is_some();
        let rough_ok = wants(&[GoalKind::Sleep]) && crate::systems::life::rough_ok(world, agent);
        let hotel = if homeless {
            crate::systems::street::hotel_for(world, agent)
        } else if !light && wants(&[GoalKind::Sleep]) {
            away_hotel.or_else(|| crate::systems::street::booked_hotel(world, agent))
        } else {
            None
        };
        let refuge =
            if home.is_none() && wants(&[GoalKind::Flee]) { crate::systems::life::refuge(world, agent) } else { None };
        let squat = world.comp::<crate::components::Squatter>(agent).map(|s| s.building);
        let derelict_target = target.filter(|&t| crate::systems::street::is_derelict(world, t));
        let gang_squat = derelict_target.is_some()
            && world.gang_of(agent).is_some_and(|g| {
                world.comp::<crate::components::Gang>(g).is_some_and(|x| x.order == crate::components::Order::Squat)
            });
        let squat_ok = derelict_target.is_some_and(|t| {
            gang_squat
                || (homeless
                    && crate::systems::street::squat_slots(world, t) > 0
                    && !crate::systems::street::squat_banned(world, agent, t))
        });
        if let (Some(o), false) = (origin, light) {
            let mut add = |key: LocationKey, b: Option<EntityId>| {
                if let Some(door) = b.and_then(|b| world.comp::<Building>(b)).map(|b| b.door) {
                    dist.insert(key, o.manhattan(door));
                }
            };
            // The island pass phase 2 (OPEN-171): a cot is a `Cot`, a Hotel a
            // `Hotel` (off, both are `Hotel`).
            let cot = crate::systems::distance::free(world).is_some()
                && hotel.is_some_and(|h| crate::systems::charity::is_mission(world, h));
            add(if cot { LocationKey::Cot } else { LocationKey::Hotel }, hotel);
            add(LocationKey::Squat, squat.or(derelict_target));
            // M13 D29: the bound seller (a Garage, a Market for a pack).
            // M13 D38: a dealer's deal Bar too.
            if target.and_then(|t| world.comp::<Building>(t)).is_some_and(|b| {
                matches!(
                    b.kind,
                    BuildingKind::Garage
                        | BuildingKind::Clinic
                        | BuildingKind::Market
                        | BuildingKind::Bar
                        // M14 V24: a deck bought at a Security Office.
                        | BuildingKind::SecurityOffice
                        // L2 L20: a dealer's front or Club.
                        | BuildingKind::Club
                        | BuildingKind::Den
                        | BuildingKind::FightPit
                )
            }) {
                add(LocationKey::Seller, target);
            }
            // M14 V5/V29: the run order's chair and a Data buyer, under Hack only.
            if wants(&[GoalKind::Hack]) {
                add(LocationKey::Chair, world.run_orders.get(&agent).map(|o| o.chair));
                add(LocationKey::DataBuyer, crate::systems::tech::data_buyer_lab(world, agent));
            }
            // M13 D38/D40: the bound Stims source.
            if stim_source {
                add(LocationKey::StimSource, target);
            }
            // M13 D34: the bound Clinic, else a Ripperdoc's employer.
            let clinic = target
                .filter(|&t| world.comp::<Building>(t).is_some_and(|b| b.kind == BuildingKind::Clinic))
                .or_else(|| {
                    job.filter(|j| j.role == Role::Ripperdoc)
                        .and_then(|_| world.resolve_building(agent, LocationKey::Clinic, None))
                });
            add(LocationKey::Clinic, clinic);
            // M13 D26: the bound vehicle's door, from the street outside it.
            if let Some(t) = target.and_then(|t| crate::systems::vehicles::vehicle_stand(world, t)) {
                dist.insert(LocationKey::Vehicle, o.manhattan(t));
            }
            // M13 D33/D36: the bound quarry (an episode's, a Harvest crew's).
            if victim_plan {
                if let Some(p) = target.and_then(|t| world.comp::<Position>(t)) {
                    let there = p.building.and_then(|b| world.comp::<Building>(b)).map_or(p.tile, |b| b.door);
                    dist.insert(LocationKey::Victim, o.manhattan(there));
                }
            }
        }

        // L1: a homeless agent's refuge (its own key's distance).
        if let (Some(o), false, Some((k, b))) = (origin, light, refuge) {
            if let Some(door) = world.comp::<Building>(b).map(|bd| bd.door) {
                dist.entry(k).or_insert(o.manhattan(door));
            }
        }

        let (target_pantry, target_occupied) = target
            .and_then(|t| world.comp::<Building>(t))
            .map_or((0, false), |b| (b.stock_food, !b.occupants.is_empty()));

        let in_gang = world.has::<GangMember>(agent);
        let shop_upgrade =
            world.comp::<crate::components::Brain>(agent).and_then(|b| b.shop_pick.as_ref()).is_some_and(|p| p.upgrade);
        let hideout_sacked = world
            .gang_of(agent)
            .and_then(|g| world.comp::<crate::components::Gang>(g))
            .is_some_and(|g| g.is_sacked(world.tick));

        let mut ctx = PlanCtx {
            agent,
            target,
            goal,
            role: job.map(|j| j.role),
            in_gang,
            adult: world.comp::<Identity>(agent).is_some_and(|i| i.age_days >= 18 * crate::time::DAYS_PER_YEAR as u32),
            lawfulness: p.map_or(0.5, |p| p.lawfulness),
            greed: p.map_or(0.5, |p| p.greed),
            pride: p.map_or(0.5, |p| p.pride),
            sociability: p.map_or(0.5, |p| p.sociability),
            courage: p.map_or(0.5, |p| p.courage),
            stealth: s.map_or(0.2, |s| s.stealth),
            farming: s.map_or(0.2, |s| s.farming),
            fighting: s.map_or(0.2, |s| crate::systems::combat::skill_power(world, s)),
            hunger,
            starving: hunger < 0.15,
            dark: world.is_dark(),
            guard8,
            season: world.season(),
            payer_negative: world.purse(payer) < 0,
            wage_at,
            // Whether wages are owed is symbolic (`has_wage_due`); the context
            // carries only today's short-payment block.
            wage_collectable: job.is_some_and(|j| j.last_wage_attempt_day != Some(day)),
            // `economy::dole_eligible`: has no Job (addendum 17).
            dole_available: crate::systems::economy::dole_eligible(world, agent)
                && world.levers.dole_per_day > 0
                && world.treasury().is_some_and(|t| t.coins >= 0)
                && world.comp::<crate::components::Brain>(agent).is_some_and(|b| b.last_dole_day != Some(day))
                // L1: a Hall trip only once the dole has accrued (or broke, or near).
                && (crate::systems::life::dole_trip_due(world, agent)),
            on_shift: job.is_some_and(|j| j.on_shift(tod)),
            evening: world.phase() == crate::time::DayPhase::Evening,
            homeless: home.is_none(),
            workplace: job
                .filter(|j| crate::exec::routine::shift_pending(world, agent, j))
                .map(|j| crate::exec::routine::workplace_key_for(world, agent, j)),
            pantry: {
                // Round 4 T1 (OPEN-231): the adults' share of it.
                let stock = home.map_or(0, |h| crate::systems::camp::adults_share(world, agent, h));
                // L2 shadow fixes item 20: at plan time, less what housemates
                // reserved on the way (several walked home to one meal).
                match home {
                    Some(h) if !light && wants(&[GoalKind::Eat]) => stock.saturating_sub(
                        crate::goap::world_state::WorldState::reserved_by_others(world, h, Some(agent)),
                    ),
                    _ => stock,
                }
            },
            target_pantry,
            target_occupied,
            market_stock: stock_of(BuildingKind::Market),
            warehouse_stock: stock_of(BuildingKind::Warehouse),
            farm_stock: farm.and_then(|f| world.comp::<Building>(f)).map_or(0, |b| b.stock_food),
            coins: world.comp::<Wallet>(agent).map_or(0, |w| w.coins),
            price: market.map_or(1, |m| crate::systems::diet::price_at(world, agent, m)).max(1),
            food_unstolen: world.comp::<crate::components::Inventory>(agent).is_some_and(|i| i.food > i.stolen_food),
            market_free: market
                .and_then(|m| world.comp::<Building>(m).map(|b| (m, b.stock_food)))
                .is_some_and(|(m, stock)| stock > WorldState::reserved_by_others(world, m, Some(agent))),
            haul_min_stock: world.config.economy.haul_min_stock,
            wage_due: job.is_some_and(|j| j.days_unpaid >= 1),
            partner: target.filter(|&t| {
                world.has::<crate::components::Brain>(t)
                    && (world
                        .comp::<Position>(t)
                        .is_some_and(|p| p.building.is_some() && p.building == pos.and_then(|q| q.building))
                        || street_beside(world, agent, t))
            }),
            street_partner: target
                .is_some_and(|t| world.has::<crate::components::Brain>(t) && street_beside(world, agent, t)),
            safe_idle: crate::systems::player::safe_idle(world, agent),
            partner_reachable: target.is_some_and(|t| {
                world.has::<crate::components::Brain>(t)
                    && world.comp::<Position>(t).and_then(|p| p.building).is_some_and(|b| {
                        Some(b) == pos.and_then(|q| q.building)
                            || world
                                .comp::<Building>(b)
                                .is_some_and(|bd| matches!(bd.kind, BuildingKind::Bar | BuildingKind::Market))
                    })
            }),
            gang_eligible: recruit.is_some(),
            extort_ok: target
                .and_then(|t| world.comp::<Building>(t))
                .is_some_and(|b| b.kind == BuildingKind::Home && !b.occupants.is_empty() && !b.derelict),
            has_loot: world.comp::<crate::components::Brain>(agent).is_some_and(|b| b.loot_today > 0),
            can_fence: world.comp::<crate::components::Inventory>(agent).is_some_and(|i| i.stolen_food > 0)
                && world
                    .gang_of(agent)
                    .and_then(|g| world.comp::<crate::components::Gang>(g))
                    .is_some_and(|g| g.treasury > 0),
            hostile_adjacent: target.is_some_and(|t| {
                world.has::<crate::components::Brain>(t) && crate::systems::law::near(world, agent, t, 4)
            }),
            corpse_target: target
                .is_some_and(|t| world.comp::<crate::components::Corpse>(t).is_some_and(|c| !c.buried)),
            may_bury: {
                let digger = job.is_some_and(|j| j.role == Role::Gravedigger);
                let kin = target.is_some_and(|t| {
                    world.edge(agent, t).is_some_and(|e| {
                        matches!(
                            e.kind,
                            crate::components::RelKind::Family
                                | crate::components::RelKind::Spouse
                                | crate::components::RelKind::Parent
                        )
                    })
                });
                let corpse = target.is_some_and(|t| world.has::<crate::components::Corpse>(t));
                digger
                    || kin
                    || corpse
                        && !world.workers(Role::Gravedigger).iter().any(|&c| world.has::<crate::components::Brain>(c))
            },
            wanted: crate::systems::law::wanted(world, agent),
            suspect_located: target.is_some_and(|t| crate::systems::law::is_located_suspect(world, t)),
            jail_day: job.is_some_and(|j| crate::systems::law::jail_duty(world, agent, j.next_shift_key(world.tick))),
            sweep: crate::systems::jobs::sweep_on(world),
            leisure_drink: crate::systems::leisure::drink_venue(world, agent),
            fun_term: crate::systems::leisure::earn_fun_term(world, agent).unwrap_or(0.0),
            patrol_pending: job.is_some_and(|j| {
                j.role == Role::Guard
                    && j.on_shift(tod)
                    && !crate::systems::law::jail_duty(world, agent, j.next_shift_key(world.tick))
                    && j.last_shift_day != Some(j.next_shift_key(world.tick))
            }),
            hall_open: matches!(
                world.phase(),
                crate::time::DayPhase::Morning | crate::time::DayPhase::Work | crate::time::DayPhase::Evening
            ),
            drank_today: crate::utility::goals::drank_today(world, agent),
            bar_full: crate::utility::goals::bar_full_for(world, agent),
            raid_pending: crate::systems::raid::raid_pending(world, agent),
            hideout_sacked,
            holes_up: crate::systems::gang::holes_up_at(world, agent).is_some(),
            can_found: crate::systems::founding::can_found(world, agent),
            hotel_available: (homeless && hotel.is_some() && crate::systems::demography::is_adult(world, agent))
                || away_hotel.is_some(),
            squatter: squat.is_some(),
            squat_ok,
            gang_squat,
            shop_pick: target.is_some_and(|t| world.has::<Building>(t))
                && world.comp::<crate::components::Brain>(agent).is_some_and(|b| b.shop_pick.is_some()),
            vehicle_target: target_vehicle
                && wants(&[GoalKind::Earn])
                && target.is_some_and(|t| crate::systems::vehicles::may_steal(world, agent, t)),
            can_fence_vehicle: (target_vehicle || held_vehicle)
                && crate::systems::vehicles::can_fence(world, agent, target),
            holds_stolen_vehicle: held_vehicle,
            steal_vehicle_cost: world.config.vehicles.steal_vehicle_cost,
            install_ready: target_clinic.is_some()
                && wants(&[GoalKind::Shop])
                && crate::systems::chrome::reserved_implant(world, agent).is_some(),
            // M13 D39: the Treat goal's Detox branch takes the Clinic visit
            // when addiction drives its score.
            therapy_affordable: treating
                && !detox_pick
                && target_clinic.is_some_and(|c| coins_now >= crate::systems::chrome::therapy_price(world, c)),
            has_implant: world.comp::<crate::components::Kit>(agent).is_some_and(|k| k.chrome),
            loot_target,
            loot_implants,
            may_rip,
            victim_target,
            episode,
            dealer,
            hideout_stims: if wants(&[GoalKind::GangWork]) {
                gang_hideout.map_or(0, |h| world.stock(h, crate::components::Good::Stims))
            } else {
                0
            },
            deal_batch: world.config.stims.deal_batch,
            stim_affordable: source_price.is_some_and(|p| coins_now >= p),
            stim_legal: stim_source
                && target.and_then(|t| world.comp::<Building>(t)).is_some_and(|b| b.kind == BuildingKind::Market),
            detox_affordable: detox_pick
                && target_clinic.is_some_and(|c| coins_now >= crate::systems::stims::detox_price(world, c)),
            withdrawal_bonus,
            misery_steal_cut: crate::systems::misery::steal_cut(world, agent).0,
            misery_lawful_cut: crate::systems::misery::steal_cut(world, agent).1,
            steal_can_buy,
            run_order: world.run_orders.contains_key(&agent)
                && world.comp::<crate::components::Kit>(agent).is_some_and(|k| k.deck.is_some()),
            has_data: crate::systems::virt::deck_data(world, agent) > 0,
            shop_upgrade,
            hideout_bed,
            away_hotel: away_hotel.is_some(),
            rough_ok,
            refuge: refuge.map(|(k, _)| k),
            poor: wants(&[GoalKind::Earn]) && crate::systems::life::earn_short(world, agent),
            beg_spot: wants(&[GoalKind::Earn]) && crate::systems::fixes::at_spot_with_company(world, agent),
            dist,
        };
        // L1: with a week of dole waiting at the Hall, scrap and begging are
        // not the plan (the cheap street chores won every broke Earn, and the
        // dole was never fetched).
        ctx.poor &= !ctx.dole_available;
        ctx
    }

    pub(super) fn is(&self, role: Role) -> bool {
        self.role == Some(role)
    }

    /// M13 review: an action serving `goals` is offered while planning one
    /// of them (any, with no goal: a step's re-check, a test).
    pub(super) fn serves(&self, goals: &[GoalKind]) -> bool {
        self.goal.is_none_or(|g| goals.contains(&g))
    }
}

/// Set one key of a world state (test and feasibility helper).
pub fn set_key(ws: &mut WorldState, key: crate::goap::world_state::Key, value: bool) {
    use crate::goap::world_state::Key as K;
    match key {
        K::HungerSatisfied => ws.hunger_satisfied = value,
        K::EnergySatisfied => ws.energy_satisfied = value,
        K::BelongingSatisfied => ws.belonging_satisfied = value,
        K::HasFood => ws.has_food = value,
        K::HasCoins => ws.has_coins = value,
        K::HasSavings => ws.has_savings = value,
        K::HasWageDue => ws.has_wage_due = value,
        K::ShiftDone => ws.shift_done = value,
        K::HasSpouse => ws.has_spouse = value,
        K::HasPartnerCandidate => ws.has_partner_candidate = value,
        K::IsSafe => ws.is_safe = value,
        K::ThreatRemoved => ws.threat_removed = value,
        K::CrimeReported => ws.crime_reported = value,
        K::SuspectJailed => ws.suspect_jailed = value,
        K::SuspectCuffed => ws.suspect_cuffed = value,
        K::InGang => ws.in_gang = value,
        K::GangTaskDone => ws.gang_task_done = value,
        K::RaidDone => ws.raid_done = value,
        K::Mustered => ws.mustered = value,
        K::CorpseBuried => ws.corpse_buried = value,
        K::CarryingCorpse => ws.carrying_corpse = value,
        K::CarryingStolen => ws.carrying_stolen = value,
        K::IsDark => ws.is_dark = value,
        K::KnownCorpse => ws.known_corpse = value,
        K::KnownSuspectLocation => ws.known_suspect_location = value,
        K::PatrolLegDone => ws.patrol_leg_done = value,
        K::FoodSourceAvailable => ws.food_source_available = value,
        K::ForageAvailable => ws.forage_available = value,
        K::Founded => ws.founded = value,
        K::CheckedIn => ws.checked_in = value,
        K::Squatting => ws.squatting = value,
        K::Bought => ws.bought = value,
        K::CarryingVehicle => ws.carrying_vehicle = value,
        K::Treated => ws.treated = value,
        K::Stripped => ws.stripped = value,
        K::HasStims => ws.has_stims = value,
        K::High => ws.high = value,
        K::RunDone => ws.run_done = value,
        K::HasData => ws.has_data = value,
    }
}
