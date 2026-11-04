use super::*;

/// Strongest against strongest until one side is out. Each pairing is an
/// Assault (or a Murder) by the raider, witnessed as usual. The vectors hold
/// whoever is still standing.
#[derive(Default)]
pub struct Tally {
    pub raider_losses: usize,
    /// Deaths on either side.
    pub deaths: usize,
    /// Defenders who lost the pairing and lived.
    pub defenders_beaten: usize,
    /// Raiders who lost a pairing and lived (a lost riot sentences them).
    pub losers: Vec<EntityId>,
    /// Bystanders hit (D35).
    pub crossfire: usize,
}

/// One brawl at `door` (D32, D35): the crossfire roll, the litter, then the
/// pairings with the kill chance × `kill_mult` (a Crush). `riot`: the
/// rioting district (its Statistical residents may be caught too, and the
/// litter is a riot's).
pub fn fight_out(
    world: &mut World,
    raiders: &mut Vec<EntityId>,
    defenders: &mut Vec<EntityId>,
    door: TilePos,
    place: &str,
    kill_mult: f32,
    riot: Option<DistrictId>,
) -> Tally {
    fight_out_until(world, raiders, defenders, door, place, kill_mult, riot, usize::MAX)
}

/// [`fight_out`], but the raiders break and scatter once `max_losses` of
/// them have lost a pairing (M12 fix pass: a corp raid's crew at a held
/// door, `[gangs] corp_raid_break`); the defenders still standing hold.
#[allow(clippy::too_many_arguments)]
pub fn fight_out_until(
    world: &mut World,
    raiders: &mut Vec<EntityId>,
    defenders: &mut Vec<EntityId>,
    door: TilePos,
    place: &str,
    kill_mult: f32,
    riot: Option<DistrictId>,
    max_losses: usize,
) -> Tally {
    let mut t = Tally::default();
    // Round 3 V12 fix: the last gate before the crossfire and the pairings: a
    // minor on either side stands out of the brawl (the pick sites filter
    // first; `law::resolve_fight_mods` counts what slips past).
    raiders.retain(|&x| !crate::systems::ages::is_protected(world, x));
    defenders.retain(|&x| !crate::systems::ages::is_protected(world, x));
    t.crossfire = open_brawl(world, raiders, defenders, door, place, riot);
    // C3a: the pairings are an engagement's duels (`Tail::Pairs`, each one's
    // tail `pair_end`), fought out here in one call.
    let mods = law::FightMods { kill_mult, why: crate::systems::combat::Purpose::Raid, ..Default::default() };
    let tail = crate::systems::combat::Tail::Pairs { door, place: place.to_string() };
    let e = crate::systems::combat::run_now(world, [raiders.clone(), defenders.clone()], mods, tail, max_losses);
    tally_of(&mut t, &e);
    *raiders = e.sides[0].clone();
    *defenders = e.sides[1].clone();
    t
}

/// The brawl's counts from its engagement (the raiders side 0).
fn tally_of(t: &mut Tally, e: &crate::systems::combat::Engagement) {
    t.raider_losses = e.losses[0];
    t.deaths = e.deaths;
    t.defenders_beaten = e.beaten[1].len();
    t.losers = e.beaten[0].clone();
}

/// C3a, a pairing's tail (`Tail::Pairs`, a raid's): the raider is the
/// aggressor: Murder when the defender died, Assault otherwise; a raider who
/// died is charged with nothing. Named before the fight (a beaten robot is
/// wrecked, despawned, in it).
pub(crate) fn pair_end(world: &mut World, door: TilePos, place: &str, c: &crate::systems::combat::Clash) {
    let (r, d) = (c.a, c.b);
    let [rn, dn] = &c.names;
    let murder = c.died && c.loser == d;
    let crime = if murder { Crime::Murder } else { Crime::Assault };
    let kind = if murder { EventKind::Murder } else { EventKind::Assault };
    let fell = if c.died && c.loser == r { " and died" } else { "" };
    let text = format!("{rn} attacked {dn} at {place}{fell}");
    world.push_event(kind, &[r, d], text);
    if law::living(world, r) {
        law::raise_crime_on(world, r, (!murder).then_some(d), Some(d), crime, door);
    }
}

/// A brawl's opening, before the pairings: the crossfire (a door nobody
/// defends has none) and, M12 D17, the street wrecked at the door (a riot's
/// mess is laid by `riot::finish`, 64 r3, whether or not it fought).
fn open_brawl(
    world: &mut World,
    raiders: &[EntityId],
    defenders: &[EntityId],
    door: TilePos,
    place: &str,
    riot: Option<DistrictId>,
) -> usize {
    let mut hits = 0;
    if let (Some(&attacker), false) = (raiders.first(), defenders.is_empty()) {
        let parties: Vec<EntityId> = raiders.iter().chain(defenders.iter()).copied().collect();
        hits = crossfire(world, door, &parties, attacker, riot, place);
    }
    if riot.is_none() && !defenders.is_empty() && !raiders.is_empty() {
        crate::systems::litter::deposit(world, door, 32, 2);
    }
    hits
}

/// D35: once per brawl, before the pairings. Every body within
/// `crossfire_radius` of the door that is not a party rolls `p_crossfire`
/// (ascending id); a riot also draws `riot_stat_bystanders` Statistical
/// residents of its district at the same odds. A hit is an Assault by
/// `attacker` (memory `CaughtInCrossfire`, safety −0.6); `p_crossfire_kill`
/// of hits kill. Returns the hits.
pub fn crossfire(
    world: &mut World,
    door: TilePos,
    parties: &[EntityId],
    attacker: EntityId,
    riot: Option<DistrictId>,
    place: &str,
) -> usize {
    let cfg = world.config.riots.clone();
    if cfg.p_crossfire <= 0.0 {
        return 0;
    }
    // Fix pass (phase 4 review): nobody on either side is a bystander: the
    // parties' gangs, any rioter of a riot among them, and (when a guard
    // fights) every guard.
    let gangs: Vec<EntityId> = parties.iter().filter_map(|&p| world.gang_of(p)).collect();
    let riots: Vec<u32> = parties.iter().filter_map(|p| world.rioter_of.get(p).copied()).collect();
    let guard_side = parties.iter().any(|&p| law::is_guard(world, p));
    let own_side = |w: &World, a: EntityId| {
        w.gang_of(a).is_some_and(|g| gangs.contains(&g))
            || w.rioter_of.get(&a).is_some_and(|r| riots.contains(r))
            || (guard_side && law::is_guard(w, a))
    };
    let mut candidates: Vec<EntityId> = world
        .bodies()
        .into_iter()
        .filter(|a| !parties.contains(a))
        .filter(|&a| !own_side(world, a))
        .filter(|&a| law::living(world, a) && !world.has::<Sentence>(a))
        // Round 4 T3 (OPEN-222): a minor is never caught in the crossfire
        // (the under-18 rule: the bystander pool had no age gate).
        .filter(|&a| !crate::systems::ages::is_protected(world, a))
        .filter(|&a| world.comp::<Position>(a).is_some_and(|p| law::chebyshev(p.tile, door) <= cfg.crossfire_radius))
        .collect();
    if let Some(d) = riot {
        let mut pool: Vec<EntityId> = world
            .districts
            .get(d.index())
            .map(|x| x.residents.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| !parties.contains(a) && !candidates.contains(a) && !own_side(world, *a))
            .filter(|&a| {
                law::living(world, a)
                    && !world.has::<Sentence>(a)
                    && !crate::systems::ages::is_protected(world, a)
                    && world.comp::<Brain>(a).is_some_and(|b| b.lod == crate::components::Lod::Statistical)
            })
            .collect();
        for _ in 0..cfg.riot_stat_bystanders {
            if pool.is_empty() {
                break;
            }
            let i = world.rng.world().random_range(0..pool.len());
            candidates.push(pool.swap_remove(i));
        }
    }
    let mut hits = 0;
    let assets = world.config.assets.enabled;
    for b in candidates {
        let roll: f32 = world.rng.world().random();
        if roll >= cfg.p_crossfire {
            continue;
        }
        // M13 D25: the bystander's dodge (reflex chrome saves from bullets too).
        if assets {
            let dodge: f32 = world.rng.world().random();
            if dodge < crate::systems::vehicles::p_dodge(world, b) {
                continue;
            }
        }
        hits += 1;
        world.stats.current.crossfire += 1;
        world.remember(b, MemoryKind::CaughtInCrossfire, Some(attacker), 0.7, -0.7, false);
        if let Some(n) = world.comp_mut::<Needs>(b) {
            n.safety = (n.safety - 0.6).max(0.0);
        }
        let kill: f32 = world.rng.world().random();
        // M13 D25: armour (Skin chrome) takes its share of the kill roll.
        let armour = if assets { world.comp::<crate::components::Kit>(b).map_or(0.0, |k| k.armour) } else { 0.0 };
        let died =
            if armour > 0.0 { kill < cfg.p_crossfire_kill * (1.0 - armour) } else { kill < cfg.p_crossfire_kill };
        let (an, bn) = (world.name_of(attacker), world.name_of(b));
        let fell = if died { " and died" } else { "" };
        let kind = if died { EventKind::Murder } else { EventKind::Assault };
        world.push_event(kind, &[attacker, b], format!("{an} hit bystander {bn} at {place}{fell}"));
        world.push_event(
            EventKind::Crossfire,
            &[attacker, b],
            format!("{bn} caught in the crossfire at {place}{fell}"),
        );
        if died {
            world.kill_by(b, crate::components::DeathCause::Violence, Some(attacker));
        }
        // Fix pass (phase 4 review): a hit is a crime as a pairing is (a
        // report, a warrant, Murder when it kills), raised after the death.
        if law::living(world, attacker) {
            let crime = if died { Crime::Murder } else { Crime::Assault };
            law::raise_crime_on(world, attacker, (!died).then_some(b), Some(b), crime, door);
        }
    }
    hits
}

/// The first raider at the Jail door resolves the breakout; later arrivals
/// find `raid_at` cleared and return `None`. Defenders are the guards inside
/// the Jail or within `raid_gather_radius` of its door. A win frees up to
/// `breakout_max_freed` of the gang's convicts, the boss first, then the
/// longest remaining sentence (`law::escape`).
pub fn breach(world: &mut World, actor: EntityId) -> Option<Outcome> {
    let gid = world.gang_of(actor)?;
    if world.comp::<Gang>(gid).is_none_or(|g| g.raid_at.is_none()) {
        return None;
    }
    let jail = world.building_of_kind(BuildingKind::Jail)?;
    let door = world.comp::<Building>(jail)?.door;
    let (gname, boss) = world.comp::<Gang>(gid).map(|g| (g.name.clone(), g.boss))?;

    let mut raiders = raiders_at(world, gid, actor, door);
    let mut defenders = jail_defenders(world, jail, door);
    robots_first(world, jail, &mut defenders);
    // Lives at work phase 3 (§ 2.2): the jailers on duty are named on the
    // breakout (a Served memory each; the first in the event's text).
    let past = crate::systems::service::jailers_passed(world, jail).map(|j| format!(", past {}", world.name_of(j)));
    let (n_raiders, n_defenders) = (raiders.len(), defenders.len());
    crate::systems::gossip::post_raid(world, Some(gid), Some(jail), door);
    // Round 3 S7 (OPEN-225): who stood at the door, for the result line.
    let (raiders0, defenders0) = (raiders.clone(), defenders.clone());
    let tally = fight_out(world, &mut raiders, &mut defenders, door, "the Precinct", 1.0, None);
    let (deaths, beaten) = (tally.deaths, tally.defenders_beaten);
    let outcome = if defenders.is_empty() { Outcome::Won } else { Outcome::Lost };

    let mut freed = Vec::new();
    if outcome == Outcome::Won {
        let now = world.tick;
        let mut convicts: Vec<(bool, Tick, EntityId)> = world
            .comp::<Gang>(gid)
            .map(|g| g.members.clone())
            .unwrap_or_default()
            .into_iter()
            // Lives at work phase 7 (§ 5.2): a member in a Station's cells is
            // not in the Precinct (the van brings him by 04:00).
            .filter(|&m| {
                world.buildings_of_kind(BuildingKind::Station).is_empty()
                    || world.comp::<crate::components::Position>(m).and_then(|p| p.building) == Some(jail)
            })
            .filter_map(|m| world.comp::<Sentence>(m).map(|s| (Some(m) != boss, s.until_tick.saturating_sub(now), m)))
            .collect();
        // The boss first (false sorts first), then the longest remaining sentence.
        convicts.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        for (_, _, m) in convicts.into_iter().take(world.config.gangs.breakout_max_freed) {
            law::escape(world, m);
            gang::push_shock(world, gid, Shock::MemberFreed);
            freed.push(m);
        }
    }
    let now = world.tick;
    match outcome {
        // Nobody inside to free: the breach fizzles, no jailbreak, no shock.
        Outcome::Won if freed.is_empty() => {}
        Outcome::Won => {
            let names: Vec<String> = freed.iter().map(|&m| world.name_of(m)).collect();
            let mut actors = vec![gid, jail];
            actors.extend(freed.iter().copied());
            world.push_event(
                EventKind::Jailbreak,
                &actors,
                format!(
                    "{gname} broke {} out of the Precinct{}: {}",
                    freed.len(),
                    past.as_deref().unwrap_or(""),
                    names.join(", ")
                ),
            );
        }
        _ => gang::push_shock(world, gid, Shock::BreakoutFailed),
    }
    if let Some(g) = world.comp_mut::<Gang>(gid) {
        g.raid_at = None;
        g.last_breakout_tick = Some(now);
    }
    let verdict = if outcome == Outcome::Won && freed.is_empty() {
        "fizzled, nobody to free".to_string()
    } else {
        format!("{outcome:?}")
    };
    world.push_event(
        EventKind::Raid,
        &[gid, jail, actor],
        format!(
            "{gname} stormed the Precinct: {verdict} ({n_raiders} raiders vs {n_defenders} guards, {deaths} dead, {} freed)",
            freed.len()
        ),
    );
    // Round 3 S7 (OPEN-225): the result every raider and guard reads (the
    // line above named only the head: a raider's diary had ten Assaults and
    // no outcome).
    crate::systems::crew::raid_result(
        world,
        crate::systems::crew::RaidLeft {
            gang: gid,
            rival: None,
            place: "the Precinct",
            raiders: &raiders0,
            defenders: &defenders0,
            raiders_standing: &raiders,
            defenders_standing: &defenders,
            won: outcome == Outcome::Won,
            coins: 0,
            food: 0,
            freed: Some(&freed),
        },
    );
    crate::systems::law::on_breakout(world, gid, outcome == Outcome::Won && !freed.is_empty(), beaten);
    faction::rethink(world, gid);
    Some(outcome)
}

/// The city guards inside the Jail or within `raid_gather_radius` of its door.
pub fn jail_defenders(world: &World, jail: EntityId, door: TilePos) -> Vec<EntityId> {
    let radius = world.config.gangs.raid_gather_radius;
    let mut defenders: Vec<EntityId> = law_brain::guards(world)
        .into_iter()
        .filter(|&g| law::living(world, g) && !crate::systems::ages::is_protected(world, g))
        .filter(|&g| {
            world
                .comp::<Position>(g)
                .is_some_and(|p| p.building == Some(jail) || law::chebyshev(p.tile, door) <= radius)
        })
        .collect();
    by_strength(world, &mut defenders);
    defenders
}

/// M13 D41: the powered robots posted at `b` stand first among its defenders.
pub fn robots_first(world: &World, b: EntityId, defenders: &mut Vec<EntityId>) {
    let robots = crate::systems::robots::defenders_at(world, b);
    if robots.is_empty() {
        return;
    }
    defenders.retain(|d| !robots.contains(d));
    defenders.splice(0..0, robots);
}

/// M14 V32: of `cands` (in removal order: robots, then summoned guards
/// weakest first), those a DoorOpen keeps from the door: taken while the
/// removed strength stays within `cap × total`; the first that would pass
/// it stops the strip (so the door at most halves the defence at 0.5).
pub fn door_strip(total: f32, cands: &[(EntityId, f32)], cap: f32) -> Vec<EntityId> {
    let limit = cap * total;
    let mut removed = Vec::new();
    let mut sum = 0.0f32;
    for &(id, s) in cands {
        if sum + s > limit {
            break;
        }
        sum += s;
        removed.push(id);
    }
    removed
}

/// M14 V33: robots at `b` turned for `gang` join its raiders, first.
pub fn turned_join(world: &World, b: EntityId, gang: EntityId, raiders: &mut Vec<EntityId>) {
    let now = world.tick;
    let turned: Vec<EntityId> = crate::systems::assets::assets_at(world, b)
        .iter()
        .copied()
        .filter(|&a| {
            world.comp::<crate::components::Asset>(a).is_some_and(|x| {
                x.kind == crate::components::AssetKind::Robot
                    && x.loc == crate::components::AssetLoc::Posted(b)
                    && x.condition > 0
                    && x.turned.is_some_and(|(side, until)| side == gang && until > now)
            })
        })
        .collect();
    if !turned.is_empty() {
        raiders.retain(|r| !turned.contains(r));
        raiders.splice(0..0, turned);
    }
}

/// `fighting + 0.25 × courage`: the brawl's pairing order.
pub fn strength(world: &World, id: EntityId) -> f32 {
    let s = crate::systems::combat::power(world, id) + 0.25 * law::courage(world, id);
    // Jobs & lives phase 3 (section 4): the side's tiers (a branch: 0 unarmed).
    let w = crate::systems::weapons::might(world, id);
    if w != 0.0 {
        s + w
    } else {
        s
    }
}

pub fn by_strength(world: &World, ids: &mut [EntityId]) {
    ids.sort_by(|&a, &b| strength(world, b).total_cmp(&strength(world, a)).then(a.cmp(&b)));
}

/// The first raider at the door resolves the raid; later arrivals find
/// `raid_at` cleared and return `None`.
pub fn brawl(world: &mut World, actor: EntityId) -> Option<Outcome> {
    let gid = world.gang_of(actor)?;
    let rival = raid_rival(world, gid)?;
    if world.comp::<Gang>(gid).is_none_or(|g| g.raid_at.is_none()) {
        return None;
    }
    // The city milestone C9b: the rival's Hideout the raid was sent to (its
    // nearest, or the seat).
    let rival_hideout = rival_target(world, gid, rival)?;
    let rival_name = world.comp::<Gang>(rival).map(|g| g.name.clone())?;
    let door = world.comp::<Building>(rival_hideout)?.door;
    let gname = world.comp::<Gang>(gid).map(|g| g.name.clone())?;

    let mut raiders = raiders_at(world, gid, actor, door);
    turned_join(world, rival_hideout, gid, &mut raiders);
    let mut defenders: Vec<EntityId> = world
        .comp::<Gang>(rival)
        .map(|g| g.members.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&m| !world.has::<Sentence>(m) && !crate::systems::ages::is_protected(world, m))
        .filter(|&m| world.comp::<Position>(m).is_some_and(|p| p.building == Some(rival_hideout)))
        .collect();
    by_strength(world, &mut defenders);
    robots_first(world, rival_hideout, &mut defenders);
    let (n_raiders, n_defenders) = (raiders.len(), defenders.len());
    // Round 3 S7: "the The Static Rats Hideout" read twice.
    let place = format!("the {} Hideout", rival_name.strip_prefix("The ").unwrap_or(&rival_name));
    let seat = crate::systems::vertical::is_seat(world, rival, rival_hideout);
    let place = if seat || !crate::systems::vertical::on(world) {
        place
    } else {
        let host = world.comp::<Building>(rival_hideout).and_then(|b| b.within).unwrap_or(rival_hideout);
        format!("{place} in {}", world.name_of(host))
    };
    let at = crate::systems::vertical::raid_where(world, rival, rival_hideout);
    crate::systems::gossip::post_raid(world, Some(gid), Some(rival), door);
    // The pressure pass phase 5b: who stood at the door, and what the Hideout held.
    let (raiders0, defenders0) = (raiders.clone(), defenders.clone());
    let chest0 = world.comp::<Gang>(rival).map_or(0, |g| g.treasury);
    let food0 = world.comp::<Building>(rival_hideout).map_or(0, |b| b.stock_food);
    // C3a: the brawl is an engagement, live when every party is Full (the
    // pairings over ticks, each one's tail `pair_end`); its end is
    // `raid_end`, the raid's tail.
    raiders.retain(|&x| !crate::systems::ages::is_protected(world, x));
    defenders.retain(|&x| !crate::systems::ages::is_protected(world, x));
    open_brawl(world, &raiders, &defenders, door, &place, None);
    let end = RaidEnd {
        gid,
        rival,
        rival_hideout,
        door,
        place,
        at,
        gname,
        rival_name,
        raiders0,
        defenders0,
        chest0,
        food0,
        seat,
        actor,
        n_raiders,
        n_defenders,
    };
    let mods = law::FightMods { why: crate::systems::combat::Purpose::Raid, ..Default::default() };
    let tail = crate::systems::combat::Tail::Raid(Box::new(end));
    world.combat.last_raid = None;
    match crate::systems::combat::engage(world, [raiders, defenders], mods, tail, true, usize::MAX) {
        crate::systems::combat::Begun::Live => None,
        crate::systems::combat::Begun::Over(_) => world.combat.last_raid.take(),
    }
}
