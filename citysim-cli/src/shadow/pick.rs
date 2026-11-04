use super::*;

/// Every candidate of an archetype, ascending by index (the CEO list is by
/// treasury, richest first). `Err` for an unknown archetype or a role the
/// build does not have.
pub(super) fn candidates(world: &World, arch: &str) -> Result<Vec<EntityId>, String> {
    let all = world.citizens();
    let adults = || all.iter().copied().filter(|&a| adult_alive(world, a));
    let by_role = |roles: &[Role]| -> Vec<EntityId> {
        adults().filter(|&a| world.comp::<Job>(a).is_some_and(|j| roles.contains(&j.role))).collect()
    };
    let mut out: Vec<EntityId> = match arch {
        "gang_member" => adults()
            .filter(|&a| {
                world.has::<GangMember>(a)
                    && world.gang_of(a).and_then(|g| world.comp::<Gang>(g)).is_some_and(|g| g.leader != Some(a))
            })
            .collect(),
        "gang_leader" => adults()
            .filter(|&a| world.gang_of(a).and_then(|g| world.comp::<Gang>(g)).is_some_and(|g| g.leader == Some(a)))
            .collect(),
        "raider" => adults()
            .filter(|&a| {
                world.gang_of(a).and_then(|g| world.comp::<Gang>(g)).is_some_and(|g| {
                    g.leader != Some(a)
                        && (g.order.is_raid() || g.pending_order.as_ref().is_some_and(|p| p.order.is_raid()))
                })
            })
            .collect(),
        "ripperdoc" => {
            let owners: BTreeSet<EntityId> = world
                .buildings_of_kind(BuildingKind::Clinic)
                .iter()
                .filter_map(|&b| world.comp::<Building>(b).and_then(|b| b.owner))
                .collect();
            // Round 5 U6 (OPEN-217): the doctors at the counter (a Ripperdoc
            // post); an owner only when nobody holds one (an absentee owner
            // with no post read as a doctor with no patients).
            let docs: Vec<EntityId> =
                adults().filter(|a| world.comp::<Job>(*a).is_some_and(|j| j.role == Role::Ripperdoc)).collect();
            if docs.is_empty() {
                adults().filter(|a| owners.contains(a)).collect()
            } else {
                docs
            }
        }
        "homeless" => adults().filter(|&a| world.comp::<Household>(a).is_some_and(|h| h.home.is_none())).collect(),
        // Shadow V4 bug 23: a cancer past stage 0 (it took one at 0.00).
        // Round 4 T7 (OPEN-233): an illness, not hunger alone: V12's 100k
        // picks were jobless and malnourished, "no cancer | sick days 0/14"
        // (the sick roll is the employed's), so S4's ladder went unwatched.
        // A cancer past stage 0 or a sick day in the last 14 (`pick` puts a
        // diagnosed cancer first); the malnourished only when nobody is ill.
        "sick" => {
            let health = |a: EntityId| world.comp::<citysim::Health>(a);
            let ill: Vec<EntityId> = adults()
                .filter(|&a| {
                    health(a).is_some_and(|h| h.sick_days != 0 || h.cancer.as_ref().is_some_and(|c| c.stage > 0.0))
                })
                .collect();
            if ill.is_empty() {
                adults().filter(|&a| health(a).is_some_and(|h| h.malnutrition >= 0.5)).collect()
            } else {
                ill
            }
        }
        "ceo" => {
            let mut corps: Vec<(i64, EntityId, EntityId)> = world
                .corps()
                .into_iter()
                .filter_map(|c| world.comp::<Corp>(c).and_then(|k| k.exec.map(|e| (k.treasury, c, e))))
                .filter(|&(_, _, e)| adult_alive(world, e))
                .collect();
            corps.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            return Ok(corps.into_iter().map(|(_, _, e)| e).collect());
        }
        "exec" => world
            .corps()
            .into_iter()
            .filter_map(|c| world.comp::<Corp>(c).and_then(|k| k.exec))
            .filter(|&e| adult_alive(world, e))
            .collect(),
        "guard" => world.guards().iter().copied().filter(|&a| adult_alive(world, a)).collect(),
        "worker" => adults().filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role != Role::Guard)).collect(),
        // V2: a deck is not a run. A `JackedIn` event naming the agent in the 7 days before today
        // (or a run seated now) qualifies; an agent with a deck and no runs does not.
        "runner" => {
            let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
            let mut ran: BTreeSet<EntityId> = world
                .events
                .iter()
                .rev()
                .take_while(|e| e.tick >= from)
                .filter(|e| e.kind == EventKind::JackedIn)
                .filter_map(|e| e.actors.first().copied())
                .collect();
            ran.extend(world.runner_of.keys().copied());
            adults().filter(|a| ran.contains(a)).collect()
        }
        "cook" => by_role(&[Role::Cook]),
        "club_staff" => by_role(&[Role::Host, Role::Attendant, Role::Croupier, Role::Concierge]),
        "fighter" => by_role(&[Role::Fighter]),
        "fabber" => by_role(&[Role::Fabber]),
        "sweeper" => by_role(&[Role::Sanitation]),
        // Jobs and room P3: a trade by its `[[trades]]` key (the night shift: the Night
        // Stockers and Night Porters); none when the config lacks the row.
        "night_shift" => {
            let roles: Vec<Role> =
                ["night_stocker", "night_porter"].iter().filter_map(|k| world.config.trade_role(k)).collect();
            by_role(&roles)
        }
        "super" | "bouncer" | "dancer" | "orderly" | "camp_warden" => {
            let roles: Vec<Role> = world.config.trade_role(arch).into_iter().collect();
            by_role(&roles)
        }
        // Jobs & lives phase 8: a trade's post holders, and (for a pitch trade) the agents
        // working its pitch; the joytoy is the chip trade's.
        "street_worker" | "vendor" | "day_labourer" | "courier" | "office_worker" | "forger" | "loader" | "builder"
        | "madam" | "companion" | "black_doc" | "teacher" | "cabbie" | "joytoy" => {
            let key = match arch {
                "cabbie" => "cab_driver",
                "joytoy" => "chip_host",
                k => k,
            };
            let Some(role) = world.config.trade_role(key) else { return Ok(Vec::new()) };
            let mut v = by_role(&[role]);
            if let Role::Trade(t) = role {
                // Shadow V4 bug 23: a pitch open (not closed that midnight) and
                // its owner free and postless (a closed pitch was picked).
                let now = world.tick;
                v.extend(
                    world
                        .jobs2
                        .pitches
                        .iter()
                        .filter(|(_, p)| p.trade == t && p.closed_until.is_none_or(|c| c <= now))
                        .map(|(&a, _)| a)
                        .filter(|&a| !world.has::<Job>(a) && !world.has::<citysim::Sentence>(a)),
                );
            }
            // Shadow V4 bug 23: at 00:00 no day post is held; the day
            // labourers are those hired off the corner in the last 7 days.
            if arch == "day_labourer" {
                let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
                let label = world.config.trade(match role {
                    Role::Trade(t) => t,
                    _ => return Ok(Vec::new()),
                });
                let label = label.map(|r| format!("hired as {}", r.label)).unwrap_or_default();
                v.extend(
                    world
                        .events
                        .iter()
                        .rev()
                        .take_while(|e| e.tick >= from)
                        .filter(|e| e.kind == EventKind::Hire && !label.is_empty() && e.text.contains(&label))
                        .filter_map(|e| e.actors.first().copied()),
                );
                // Y3: at 100k the event ring holds under a day of Hires, so the
                // window finds nobody; the corner's own pool (the free, jobless,
                // broke adults the dawn pick queues, on screen) stands in.
                if v.is_empty() {
                    v.extend(
                        world
                            .citizens()
                            .into_iter()
                            .filter(|&a| citysim::systems::streetwork::corner_ok(world, a))
                            .filter(|&a| {
                                world
                                    .comp::<citysim::components::Brain>(a)
                                    .is_some_and(|b| b.lod != citysim::components::Lod::Statistical)
                            })
                            .filter(|&a| {
                                citysim::systems::careers::admits(
                                    world,
                                    a,
                                    citysim::systems::careers::hire_band(world, role),
                                )
                            }),
                    );
                }
                v.sort_unstable();
                v.dedup();
            }
            v.into_iter().filter(|&a| adult_alive(world, a)).collect()
        }
        // Shadow V4 bug 23: a free pimp (a jailed one spent the week inside).
        "pimp" => world
            .jobs2
            .vice
            .pimps
            .values()
            .copied()
            .filter(|&a| adult_alive(world, a) && !world.has::<citysim::Sentence>(a))
            .collect(),
        "foreman" => {
            let Some(role) = world.config.trade_role("builder") else { return Ok(Vec::new()) };
            let yards: BTreeSet<EntityId> =
                by_role(&[role]).into_iter().filter_map(|a| world.comp::<Job>(a).and_then(|j| j.employer)).collect();
            yards.into_iter().filter_map(|y| citysim::systems::industry::foreman(world, y, None)).collect()
        }
        // A gang dealer whose gang keeps weapons in its Hideout (the dealer over goods).
        // Round 8 X6 (OPEN-217): and a dealer who sold a weapon in the 7 days
        // before today (a weapon whose provenance is his deal; `pick` puts
        // them first): V12-V16 had no arms_dealer diary while `sold_dealer`
        // counted the trade.
        "arms_dealer" => {
            let mut dealers: BTreeSet<EntityId> = world.dealers.values().flatten().copied().collect();
            dealers.extend(world.deal_log.values().map(|&(a, _)| a));
            let dealt = arms_dealt(world);
            adults()
                .filter(|a| dealt.contains(a) || dealers.contains(a))
                // Shadow V4 bug 23: a surplus to sell (the armoury's target
                // kept back), not any stock.
                .filter(|&a| {
                    dealt.contains(&a)
                        || world.gang_of(a).is_some_and(|g| citysim::systems::weapons::has_surplus(world, g))
                })
                .collect()
        }
        "elder" => {
            let from = world.config.ages.frail_from_years;
            adults().filter(|&a| world.comp::<Identity>(a).is_some_and(|i| i.age_years() >= from)).collect()
        }
        // A member of a gang under a `[faith]` cult row.
        "cultist" => adults()
            .filter(|&a| {
                world.gang_of(a).and_then(|g| world.comp::<Gang>(g)).and_then(|g| g.creed).is_some_and(|c| match c {
                    citysim::word::Creed::Config(i) => {
                        world.config.faith.creeds.get(usize::from(i)).is_some_and(|r| r.cult)
                    }
                    citysim::word::Creed::Purist => false,
                })
            })
            .collect(),
        // A member of a gang typed with organ targets (the scavs).
        "scav" => adults()
            .filter(|&a| world.has::<GangMember>(a))
            .filter(|&a| world.gang_of(a).is_some_and(|g| citysim::systems::gray::organ_targets(world, g)))
            .collect(),
        // Round 4 T5: the searching jobless, the freshest post lost first (then id).
        "bereaved" => {
            let mut v: Vec<(std::cmp::Reverse<u64>, EntityId)> = adults()
                .filter_map(|a| citysim::systems::grief::mourning(world, a).map(|m| (std::cmp::Reverse(m.since), a)))
                .collect();
            v.sort();
            v.into_iter().map(|(_, a)| a).collect()
        }
        "marked" => {
            let mut v: Vec<(bool, EntityId)> = world
                .contracts
                .values()
                .filter(|c| c.status == citysim::contract::ContractStatus::Taken && c.kind.violent())
                .filter_map(|c| c.target_agent())
                .filter(|&t| adult_alive(world, t))
                .map(|t| (!citysim::systems::grief::lying_low(world, t), t))
                .collect();
            v.sort();
            v.dedup();
            v.into_iter().map(|(_, a)| a).collect()
        }
        "jobless" => {
            let mut v: Vec<(u64, EntityId)> = adults()
                .filter(|&a| citysim::systems::search::searching(world, a))
                .map(|a| (citysim::systems::search::idle_days(world, a), a))
                .collect();
            v.sort();
            v.into_iter().map(|(_, a)| a).collect()
        }
        // V2: the diary should start on the weekday before `[leisure] collect_weekday` (Friday on
        // the shipped config: weekday 5 of `day % 7`, the last workday before the rest day 6).
        "worker_friday" => adults()
            .filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role != Role::Guard))
            .filter(|&a| class_of(world, a) == citysim::Class::Street)
            .collect(),
        "purist" => adults().filter(|&a| world.has::<GangMember>(a) && is_purist(world, a)).collect(),
        "reporter" => adults().filter(|&a| world.comp::<Job>(a).is_some_and(|j| j.role == Role::Reporter)).collect(),
        "child" => {
            let kids: Vec<EntityId> = all
                .iter()
                .copied()
                .filter(|&a| world.has::<Child>(a) && world.has::<Identity>(a) && !world.has::<Corpse>(a))
                .collect();
            // Infants do nothing readable: prefer a child old enough to have a life.
            let grown: Vec<EntityId> = kids
                .iter()
                .copied()
                .filter(|&a| world.comp::<Identity>(a).is_some_and(|i| i.age_years() >= 5))
                .collect();
            if grown.is_empty() {
                kids
            } else {
                grown
            }
        }
        // V2: one candidate per dealer, not per Bar: every dealer in a `deal_log` row (a Bar's
        // last dealer, within 7 days, Statistical hits included) and every dealer registered at a
        // Bar now. No gang-member fallback: a "dealer" who never dealt is no dealer.
        "dealer" => {
            let today = world.day();
            let mut dealers: BTreeSet<EntityId> =
                world.deal_log.values().filter(|&&(_, d)| d + 7 >= today).map(|&(a, _)| a).collect();
            dealers.extend(world.dealers.values().flatten().copied());
            adults().filter(|a| dealers.contains(a) && world.has::<GangMember>(*a)).collect()
        }
        // M16a 5.3: whoever runs a Fixer's office: its owner (an agent) or a hired Fixer.
        // Round 5 U5 (OPEN-217): an owner with a post elsewhere is not running it (V13's
        // fixer_23959 a Vat Tech, U5's base fixer_21976 a Street Market clerk).
        "fixer" => {
            let owners: BTreeSet<EntityId> = world
                .buildings_of_kind(BuildingKind::Fixer)
                .iter()
                .filter_map(|&b| world.comp::<Building>(b).and_then(|b| b.owner))
                .collect();
            adults()
                .filter(|a| match world.comp::<Job>(*a) {
                    Some(j) => j.role == Role::Fixer,
                    None => owners.contains(a),
                })
                .collect()
        }
        // M16a 5.3: an agent holding a taken contract (its taker or a crew member) or one that
        // fulfilled a contract in the 7 days before today (closed records stay 30 days).
        // Round 8 X6 (OPEN-217): a gun's contract is violence (a Hit or a Beat; V16's two
        // picks were corp techs on a Tag and a Steal, no weapon); `pick` puts a record taken
        // now first, then the armed.
        "gun" => {
            let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
            let mut guns: BTreeSet<EntityId> = BTreeSet::new();
            for c in world.contracts.values().filter(|c| c.kind.violent()) {
                let held = c.status == citysim::contract::ContractStatus::Taken;
                let done =
                    c.status == citysim::contract::ContractStatus::Fulfilled && c.closed.is_some_and(|t| t >= from);
                if held || done {
                    guns.extend(c.taker);
                    guns.extend(c.crew.iter().copied());
                }
            }
            adults().filter(|a| guns.contains(a)).collect()
        }
        // The Hit market phase 4: a want's holder, or the placing agent of an organic record
        // (a motive set) still live or posted in the 7 days before today.
        "buyer" => {
            let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
            let mut buyers: BTreeSet<EntityId> = world.wants.keys().copied().collect();
            for c in world.contracts.values().filter(|c| !c.motive.is_none()) {
                if c.status.is_live() || c.posted >= from {
                    buyers.extend(c.agent);
                }
            }
            adults().filter(|a| buyers.contains(a)).collect()
        }
        // The pressure pass phase 1 (OPEN-178): the adults below `[misery] mood_line` (any adult
        // below 0 without the section); `pick` orders them lowest mood first.
        "miserable" => {
            let line = world.config.misery.as_ref().map_or(0.0, |c| c.mood_line);
            adults().filter(|&a| world.comp::<citysim::components::Mood>(a).is_some_and(|m| m.value < line)).collect()
        }
        "owner_broke" => citysim::systems::underwater::owners_by_till(world).into_iter().map(|(_, o)| o).collect(),
        "jailed" => adults().filter(|&a| world.has::<Sentence>(a)).collect(),
        "dodger" => adults()
            .filter(|a| world.fares.book.get(a).is_some_and(|r| r.caught.len() >= 2 || r.banned_until.is_some()))
            .collect(),
        "school_parent" => {
            // The pressure pass S6 (OPEN-224): a parent with a job whose child
            // is on a School's roll today or has been this childhood (a start
            // day after the rest day still holds the rest day's Play).
            let mut v = Vec::new();
            for &k in all.iter() {
                if !world.has::<Child>(k) || world.has::<Corpse>(k) {
                    continue;
                }
                let at_school = matches!(
                    citysim::systems::ages::place_of(world, k),
                    Some(citysim::systems::ages::Place::School(_))
                );
                if at_school || world.comp::<Child>(k).is_some_and(|c| c.edu_days > 0) {
                    v.extend(citysim::systems::camp::parents_of(world, k));
                }
            }
            v.retain(|&a| world.has::<Brain>(a) && world.has::<Job>(a) && !world.has::<Sentence>(a));
            v
        }
        "camp_parent" => {
            let mut v = Vec::new();
            for camp in citysim::systems::camp::standing_camps(world) {
                for &k in citysim::systems::camp::children_of(world, camp) {
                    v.extend(citysim::systems::camp::parents_of(world, k));
                }
            }
            // Round 4 T1 (OPEN-231): and a parent whose Home is on the
            // service's warning (the ladder's first rung).
            for &k in all.iter() {
                let home = world.comp::<citysim::Household>(k).and_then(|h| h.home);
                if world.has::<Child>(k) && home.is_some_and(|h| citysim::systems::camp::home_on_warning(world, h)) {
                    v.extend(citysim::systems::camp::parents_of(world, k));
                }
            }
            v.retain(|&a| world.has::<Brain>(a) && !world.has::<Sentence>(a) && !world.has::<Corpse>(a));
            v
        }
        "neglect_parent" => {
            let mut v = Vec::new();
            for &k in all.iter() {
                let home = world.comp::<citysim::Household>(k).and_then(|h| h.home);
                let at_home =
                    home.and_then(|h| world.comp::<Building>(h)).is_some_and(|b| b.kind == BuildingKind::Home);
                if at_home && world.comp::<Child>(k).is_some_and(|c| c.hunger_days >= 1) {
                    v.extend(citysim::systems::camp::parents_of(world, k));
                }
            }
            v.retain(|&a| world.has::<Brain>(a) && !world.has::<Corpse>(a));
            v
        }
        "camp_jobless" => {
            let mut v = Vec::new();
            for camp in citysim::systems::camp::standing_camps(world) {
                for &k in citysim::systems::camp::children_of(world, camp) {
                    v.extend(citysim::systems::camp::parents_of(world, k));
                }
            }
            v.retain(|&a| {
                world.has::<Brain>(a) && !world.has::<Job>(a) && !world.has::<Sentence>(a) && !world.has::<Corpse>(a)
            });
            v
        }
        other => return Err(format!("unknown archetype {other:?} (known: {})", ARCHETYPES.join(", "))),
    };
    out.sort();
    out.dedup();
    Ok(out)
}

/// Fisher-Yates keyed on the seed and the archetype name: the library's
/// `rng::splitmix64` over an FNV key and the swap index (no second copy of
/// the hash constants here).
pub(super) fn shuffled(mut v: Vec<EntityId>, seed: u64, arch: &str) -> Vec<EntityId> {
    let key =
        seed ^ arch.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    for i in (1..v.len()).rev() {
        let j = (citysim::rng::splitmix64(key ^ ((i as u64) << 32)) % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
    v
}

/// Round 8 X6 (OPEN-217): `pick`'s mark for an archetype with no candidate.
pub(super) const EMPTY_PICK: &str = "!empty:";

/// Round 8 X6 (OPEN-217): the dealers who sold a weapon in the 7 days before
/// today (a weapon's provenance `Dealt` by him).
fn arms_dealt(world: &World) -> BTreeSet<EntityId> {
    let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
    world
        .with::<citysim::components::Asset>()
        .into_iter()
        .filter_map(|a| world.comp::<citysim::components::Asset>(a).and_then(|x| x.prov))
        .filter(|p| p.how == citysim::components::ProvKind::Dealt && p.at >= from)
        .filter_map(|p| p.by)
        .collect()
}

/// Round 8 X6 (OPEN-217): the archetypes whose picks are ranked by how much
/// they do its thing (lower first). `gun`: on a taken record now, then armed;
/// `runner`: a gang member, then the most runs in the 7 days before today;
/// `arms_dealer`: a weapon dealt in those 7 days, then a gang member;
/// `jailed`: the most of the term left (a week inside to read).
#[allow(clippy::type_complexity)]
fn doer_rank<'a>(world: &'a World, arch: &str) -> Option<Box<dyn Fn(EntityId) -> (u8, u8, i64) + 'a>> {
    match arch {
        "gun" => {
            let live: BTreeSet<EntityId> = world
                .contracts
                .values()
                .filter(|c| c.kind.violent() && c.status == citysim::contract::ContractStatus::Taken)
                .flat_map(|c| c.taker.into_iter().chain(c.crew.iter().copied()))
                .collect();
            Some(Box::new(move |a| {
                let armed = citysim::systems::weapons::tier(world, a) > 0;
                (u8::from(!live.contains(&a)), u8::from(!armed), 0)
            }))
        }
        "runner" => {
            let from = world.tick.saturating_sub(7 * TICKS_PER_DAY);
            let mut runs: std::collections::BTreeMap<EntityId, i64> = std::collections::BTreeMap::new();
            for e in world.events.iter().rev().take_while(|e| e.tick >= from) {
                if e.kind == EventKind::JackedIn {
                    if let Some(&a) = e.actors.first() {
                        *runs.entry(a).or_default() += 1;
                    }
                }
            }
            Some(Box::new(move |a| (u8::from(!world.has::<GangMember>(a)), 0, -runs.get(&a).copied().unwrap_or(0))))
        }
        "arms_dealer" => {
            let dealt = arms_dealt(world);
            Some(Box::new(move |a| (u8::from(!dealt.contains(&a)), u8::from(!world.has::<GangMember>(a)), 0)))
        }
        "jailed" => Some(Box::new(move |a| {
            let left = world.comp::<Sentence>(a).map_or(0, |s| s.until_tick.saturating_sub(world.tick));
            (0, 0, -i64::try_from(left).unwrap_or(i64::MAX))
        })),
        _ => None,
    }
}

/// The world of `seed` run normally to the start of `start_day`. Shadow V6:
/// `map` overrides `[world] map` as `run --map` does (with `population`,
/// the scaled config on that map instead of the generated one).
pub(super) fn start_world(
    seed: u64,
    start_day: u64,
    population: Option<u32>,
    map: Option<&std::path::Path>,
) -> Result<World, String> {
    let mut config = match population {
        Some(n) => {
            let mut c = Config::load().scaled_to(n);
            c.world.map = format!("gen:{n}");
            c.ambient.flats_per_slot = 50.0;
            c
        }
        None => Config::load(),
    };
    if let Some(m) = map {
        config.world.map = crate::map_path(m)?;
    }
    let mut world = World::new(seed, config);
    let start_tick = start_day * TICKS_PER_DAY;
    while world.tick < start_tick {
        citysim::tick(&mut world);
    }
    Ok(world)
}

/// The picks: `count` per archetype of the comma-separated `list` (free
/// agents first, shuffled by the seed; the CEO list by treasury), then every
/// live `--agent` index.
pub(super) fn pick(
    world: &World,
    seed: u64,
    start_day: u64,
    list: Option<&str>,
    count: usize,
    agents: &[u32],
) -> Vec<(String, EntityId)> {
    let mut picked: Vec<(String, EntityId)> = Vec::new();
    let mut taken: BTreeSet<EntityId> = BTreeSet::new();
    if let Some(list) = list {
        for tok in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            // Shadow V7 (tool): `arch:N` picks N of this archetype instead of `--count`.
            let (arch, count) = match tok.split_once(':') {
                Some((a, n)) => (a, n.parse::<usize>().unwrap_or(count)),
                None => (tok, count),
            };
            match candidates(world, arch) {
                Err(e) => eprintln!("note: {e}"),
                // Round 8 X6 (OPEN-217): a tool asserts its preconditions: an empty
                // pick is an error line, and the run ends in an error once the
                // other diaries are written (`shadow`).
                Ok(c) if c.is_empty() => {
                    eprintln!("error: no {arch} candidates on seed {seed} at day {start_day} (nobody does its thing)");
                    picked.push((format!("{EMPTY_PICK}{arch}"), EntityId::NONE));
                }
                Ok(_) if arch == "child" => {
                    eprintln!("note: child is unpinnable (no Brain, no Full LOD); not picked (see --list)")
                }
                Ok(c) => {
                    if arch == "worker_friday" {
                        let friday = (world.config.leisure.collect_weekday + 6) % 7;
                        if start_day % 7 != friday {
                            eprintln!(
                                "note: worker_friday: day {start_day} is weekday {}, not the Friday (weekday {friday}); the diary does not start on the Friday",
                                start_day % 7
                            );
                        }
                    }
                    let ordered = if arch == "ceo" {
                        c
                    } else if arch == "owner_broke" {
                        let by = citysim::systems::underwater::owners_by_till(world);
                        eprintln!(
                            "note: owner_broke: {} owners with staff; the lowest till {:.2} weeks of wages",
                            by.len(),
                            by.first().map_or(0.0, |x| x.0)
                        );
                        by.into_iter().map(|(_, o)| o).filter(|o| c.contains(o)).collect()
                    } else if arch == "miserable" {
                        let mood = |a: &EntityId| world.comp::<citysim::components::Mood>(*a).map_or(0.0, |m| m.value);
                        let mut c = c;
                        c.sort_by(|a, b| mood(a).total_cmp(&mood(b)).then(a.cmp(b)));
                        let adults = world.citizens().into_iter().filter(|&a| adult_alive(world, a)).count();
                        eprintln!(
                            "note: miserable: {} of {adults} adults under the misery line; the lowest mood {:+.2}",
                            c.len(),
                            c.first().map_or(0.0, mood)
                        );
                        c
                    } else if arch == "sick" {
                        // Round 4 T7: a diagnosed cancer first (the care it asks for).
                        let mut c = shuffled(c, seed, arch);
                        c.sort_by_key(|&a| {
                            !world
                                .comp::<citysim::Health>(a)
                                .and_then(|h| h.cancer.as_ref())
                                .is_some_and(|k| k.diagnosed && k.stage > 0.0)
                        });
                        c
                    } else if arch == "jobless" {
                        // Round 4 T5: the candidates' order (the freshest post lost first).
                        c
                    } else if let Some(rank) = doer_rank(world, arch) {
                        // Round 8 X6 (OPEN-217): the ones who do the archetype's thing
                        // most first (shuffled within a rank).
                        let mut c = shuffled(c, seed, arch);
                        c.sort_by_key(|&a| rank(a));
                        c
                    } else {
                        shuffled(c, seed, arch)
                    };
                    // L1 (the V1 tool note): free agents first; a jailed pick's
                    // week is the cells (V1's dealer and leader were inside).
                    let (free, jailed): (Vec<EntityId>, Vec<EntityId>) =
                        ordered.into_iter().partition(|&id| is_free(world, id));
                    let ordered: Vec<EntityId> = free.into_iter().chain(jailed).collect();
                    let mut n = 0;
                    for id in ordered {
                        if n >= count {
                            break;
                        }
                        if taken.insert(id) {
                            picked.push((arch.to_string(), id));
                            n += 1;
                        }
                    }
                    if n < count {
                        eprintln!("note: only {n} distinct {arch} candidate(s) (asked for {count})");
                    }
                }
            }
        }
    }
    for &ix in agents {
        match world.citizens().into_iter().find(|c| c.index == ix) {
            Some(id) if world.has::<Identity>(id) && !world.has::<Corpse>(id) => {
                if taken.insert(id) {
                    picked.push(("agent".into(), id));
                }
            }
            _ => eprintln!("note: agent {ix} is not alive at day {start_day}"),
        }
    }
    picked
}

/// Follow the picks for `days` from the world's tick, pinned unless
/// `pin` is false; the tracks come back closed (the last day flushed).
/// The city milestone C2: the window's saves (`--save-at`): at each of `ticks` the world goes to
/// `dir/<seed>-<tick>.ron` before the tick runs (the picks pinned, as the diary's world).
pub(super) struct Saves {
    pub(super) ticks: Vec<Tick>,
    pub(super) dir: PathBuf,
    pub(super) seed: u64,
}

pub(super) fn follow(
    world: &mut World,
    picked: &[(String, EntityId)],
    days: u64,
    pin: bool,
    verbose: bool,
) -> Vec<Track> {
    follow_saving(world, picked, days, pin, verbose, None, None).expect("no saves, no I/O")
}

pub(super) fn follow_saving(
    world: &mut World,
    picked: &[(String, EntityId)],
    days: u64,
    pin: bool,
    verbose: bool,
    saves: Option<&Saves>,
    mut recorder: Option<&mut crate::record::Recorder>,
) -> Result<Vec<Track>, String> {
    let start_tick = world.tick;
    world.shadow_notes = Some(Vec::new());
    // The view milestone V7: the picks' crossings and jaywalks (`[cross]` lines).
    world.crossing_watch = Some(picked.iter().map(|(_, id)| (*id, Vec::new())).collect());
    let mut tracks: Vec<Track> = Vec::new();
    for (arch, id) in picked {
        let name = world.comp::<Identity>(*id).map_or("?".into(), |i| i.name.clone());
        let mut t = Track::new(*id, arch, name, start_tick);
        t.pinned = pin;
        t.spouse = world.spouse_of(*id);
        t.header = header(world, &t);
        t.last_place = place(world, *id);
        if verbose {
            eprintln!("shadowing {arch}: {} (#{})", t.name, id.index);
        }
        tracks.push(t);
    }
    let end_tick = start_tick + days * TICKS_PER_DAY;
    let save_now = |world: &mut World| -> Result<(), String> {
        if let Some(s) = saves.filter(|s| s.ticks.contains(&world.tick)) {
            // C3a: the live fights end before the save (engagements are not saved).
            citysim::systems::combat::settle_all(world);
            let path = citysim::save::save_path(&s.dir, s.seed, world.tick);
            citysim::save::save_to_file(world, &path).map_err(|e| format!("save {}: {e}", path.display()))?;
            eprintln!("saved {}", path.display());
        }
        Ok(())
    };
    while world.tick < end_tick {
        if pin {
            for t in &tracks {
                if let Some(b) = world.comp_mut::<Brain>(t.id) {
                    b.pinned = true;
                }
            }
        }
        save_now(world)?;
        if let Some(r) = recorder.as_deref() {
            r.before_tick(world);
        }
        let cursor = world.next_event_id;
        if let Some(n) = world.shadow_notes.as_mut() {
            n.clear();
        }
        citysim::tick(world);
        let now = world.tick - 1;
        let mut events: Vec<&Event> = world.events.iter().rev().take_while(|e| e.id >= cursor).collect();
        events.reverse();
        let dbg: Vec<&Event> = world.debug_events.iter().rev().take_while(|e| e.tick == now).collect();
        let notes: Vec<ShadowNote> = world.shadow_notes.clone().unwrap_or_default();
        for t in &mut tracks {
            observe(world, t, now, &events, &dbg, &notes);
        }
        if let Some(r) = recorder.as_deref_mut() {
            r.after_tick(world)?;
        }
    }
    save_now(world)?;
    let end = world.tick;
    for t in &mut tracks {
        // The last day's summary and any open action.
        close_action(world, t, end, "run ended");
        flush_day(world, t, end);
        end_sleep_block(t, end);
        close_still(t, end);
    }
    Ok(tracks)
}
