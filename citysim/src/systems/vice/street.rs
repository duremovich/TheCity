use super::*;

/// A Pitch of a `vice` row (the street worker's).
pub fn is_street_pitch(world: &World, p: &Pitch) -> bool {
    world.config.trade(p.trade).is_some_and(|r| r.vice && r.workplace.is_empty())
}

/// `streetwork::close_the_day`'s sweep of a street pitch: never with
/// `street_legal` on; in a `Sweep` district on `p_street_sweep`; with
/// `patrol_sweep` on (phase 5's carry-over), in a district the law patrols
/// or cracks down on with guards allocated, on `p_patrol_sweep` × the law's
/// alertness (M16b's attention). Files `Solicitation` while under the day's
/// cap. Returns whether it is swept (the caller fines and closes it, under
/// `[jobs2] sweeps_per_day`).
pub fn sweep_street(world: &mut World, p: &Pitch) -> bool {
    if world.levers.vice.street_legal {
        return false;
    }
    let d = world.districts.get(p.district.index());
    let sweep = d.is_some_and(|d| d.stance == Stance::Sweep);
    let u = hash(world, PURPOSE_SWEEP, key(p.owner), world.day());
    let swept = if sweep {
        u < world.config.vice.p_street_sweep
    } else if world.config.vice.patrol_sweep
        && d.is_some_and(|d| matches!(d.stance, Stance::Patrol | Stance::Crackdown(_)) && d.guards > 0)
    {
        u < world.config.vice.p_patrol_sweep * crate::systems::attention::alertness_mult(world, None)
    } else {
        false
    };
    if !swept {
        return false;
    }
    if !sweep {
        ownership::jobs2_row(world).patrol_sweeps += 1;
    }
    let cap = crate::systems::weapons::per_capita_day(world, world.config.vice.solicit_per_1000_day);
    if world.jobs2.vice.solicit_today < cap {
        world.jobs2.vice.solicit_today += 1;
        ownership::jobs2_row(world).solicitation += 1;
        crate::systems::law::file_report(world, crate::components::Crime::Solicitation, p.owner, None);
    }
    true
}

/// Hourly: each working street pitch's visit roll and, unprotected, its
/// assault roll. O(street pitches).
pub(super) fn street_hour(world: &mut World) {
    let cfg = world.config.vice.clone();
    let hour = world.tick / TICKS_PER_HOUR;
    let owners: Vec<EntityId> = world.jobs2.pitches.keys().copied().collect();
    for o in owners {
        let Some(p) = world.jobs2.pitches.get(&o).cloned() else { continue };
        if !is_street_pitch(world, &p) || !crate::systems::streetwork::working(world, &p) {
            continue;
        }
        let street = crate::systems::service::street_on(world);
        let mut client = None;
        if hash(world, PURPOSE_VISIT, key(o), hour) < cfg.p_street_visit {
            // Lives at work phase 4 (§ 2.3): a body near the pitch first.
            let near = if street { client_near(world, &p, hour) } else { None };
            if near.is_some() {
                ownership::jobs2_row(world).street_clients_near += 1;
            }
            if let Some(payer) = near.or_else(|| street_payer(world, &p, hour)) {
                let paid = ownership::pay(world, Some(payer), Some(o), cfg.street_price, Flow::Leisure);
                if paid > 0 {
                    add_intimacy(world, payer, cfg.intimacy_gain);
                    if let Some(x) = world.jobs2.pitches.get_mut(&o) {
                        x.take_today += paid;
                    }
                    let row = ownership::jobs2_row(world);
                    row.street_visits += 1;
                    row.street_take += paid;
                    if street {
                        client = Some(payer);
                        let kind = crate::components::ServeKind::Client;
                        crate::systems::service::encounter(world, o, payer, None, p.district, kind, 0);
                    }
                }
            }
        }
        // Lives at work phase 4b (§ 2.3): the pimp's presence, not his
        // freedom; a Statistical pimp's round is credited here.
        let pimp = if street {
            let here = pimp_present(world, p.district, p.spot);
            if here {
                stat_collection(world, &p);
            }
            here
        } else {
            pimp_of(world, p.district).is_some()
        };
        let protected = pimp || world.comp::<Kit>(o).is_some_and(|k| k.weapon > 0);
        if protected {
            continue;
        }
        let fear = world.districts.get(p.district.index()).map_or(0.0, |d| d.fear);
        if hash(world, PURPOSE_ASSAULT, key(o), hour) < cfg.p_assault_street * (0.5 + fear) {
            street_assault(world, o, &p, hour, client);
        }
    }
}

/// Lives at work phase 4 (§ 2.3, the client): a body out near the pitch
/// (within `client_tiles` of its spot, Full or Coarse, in no building):
/// an adult, free, not the worker, with the visit's wants and twice the
/// price (`street_payer`'s rule), the lowest hash of the hour (D6).
fn client_near(world: &mut World, p: &Pitch, hour: u64) -> Option<EntityId> {
    let r = world.config.service.client_tiles;
    let near = world.agents_near(p.spot, r, None);
    let cfg = &world.config.vice;
    near.into_iter()
        .filter(|&id| {
            id != p.owner
                && world.comp::<Brain>(id).is_some_and(|b| b.lod != Lod::Statistical)
                && world.comp::<Position>(id).is_some_and(|x| x.building.is_none())
                && crate::systems::demography::is_adult(world, id)
                // Round 5 U6: the under-18 rule named, not leant on the adult age.
                && !crate::systems::ages::is_protected(world, id)
                && !world.has::<Sentence>(id)
                && crate::systems::law::living(world, id)
                && world.comp::<Needs>(id).is_some_and(|n| n.intimacy < cfg.street_intimacy && n.hunger >= 0.5)
                && world.comp::<Wallet>(id).is_some_and(|w| w.coins >= 2 * cfg.street_price)
        })
        .map(|id| (hash(world, PURPOSE_CLIENT, key(p.owner) ^ key(id), hour).to_bits(), id))
        .min()
        .map(|(_, id)| id)
}

/// A resident adult of the pitch's district (three draws by a hash) with
/// the price, `intimacy` under `street_intimacy` and `hunger` at 0.5 or
/// more (fed first), not the worker.
pub(super) fn street_payer(world: &World, p: &Pitch, hour: u64) -> Option<EntityId> {
    let res = &world.districts.get(p.district.index())?.residents;
    if res.is_empty() {
        return None;
    }
    let cfg = &world.config.vice;
    // The pre-view pass phase 4 (P3): a body pays only in person
    // (`client_near`); the drawn payer is an off-screen resident.
    let presence = crate::systems::presence::on(world);
    (0..3u64).find_map(|k| {
        let u = hash(world, PURPOSE_PAYER, key(p.owner), hour * 4 + k);
        let id = res[((u * res.len() as f32) as usize).min(res.len() - 1)];
        (id != p.owner
            && !(presence && crate::systems::presence::is_body(world, id))
            && crate::systems::demography::is_adult(world, id)
            && !crate::systems::ages::is_protected(world, id)
            && !world.has::<Sentence>(id)
            // A hungry body buys food first (no visit under 0.5 hunger).
            && world.comp::<Needs>(id).is_some_and(|n| n.intimacy < cfg.street_intimacy && n.hunger >= 0.5)
            && world.comp::<Wallet>(id).is_some_and(|w| w.coins >= 2 * cfg.street_price))
        .then_some(id)
    })
}

/// An unprotected street pitch's assault: a lawless resident of the
/// district (the first by a hash, else none) takes the night's coins in
/// the worker's wallet; the worker's memories and safety.
///
/// Lives at work phase 4 (§ 2.3): the hour's `client`, when lawless, is the
/// actor (the robbery is one of the client's outcomes); else the hashed pick.
fn street_assault(world: &mut World, o: EntityId, p: &Pitch, hour: u64, client: Option<EntityId>) {
    let lawless =
        |w: &World, a: EntityId| w.comp::<crate::components::Personality>(a).is_some_and(|x| x.lawfulness < 0.3);
    let by_client = client.filter(|&c| lawless(world, c) && !world.has::<Sentence>(c));
    let actor = match by_client {
        Some(c) => Some(c),
        None => {
            let Some(res) = world.districts.get(p.district.index()).map(|d| d.residents.clone()) else { return };
            if res.is_empty() {
                return;
            }
            let u = hash(world, PURPOSE_PAYER, key(o), hour * 4 + 3);
            let start = ((u * res.len() as f32) as usize).min(res.len() - 1);
            (0..res.len()).map(|k| res[(start + k) % res.len()]).find(|&a| {
                a != o
                    && crate::systems::demography::is_adult(world, a)
                    && !world.has::<Sentence>(a)
                    && lawless(world, a)
            })
        }
    };
    let Some(a) = actor else { return };
    let take = p.take_today.min(world.comp::<Wallet>(o).map_or(0, |w| w.coins.max(0)));
    if take > 0 {
        if let Some(w) = world.comp_mut::<Wallet>(o) {
            w.coins -= take;
        }
        if let Some(w) = world.comp_mut::<Wallet>(a) {
            w.coins += take;
        }
    }
    world.remember(o, MemoryKind::WasRobbed, Some(a), 0.6, -0.6, false);
    world.remember(o, MemoryKind::Lost, Some(a), 0.6, -0.6, false);
    if let Some(n) = world.comp_mut::<Needs>(o) {
        n.safety = (n.safety - 0.4).max(0.0);
    }
    ownership::jobs2_row(world).street_assaults += 1;
    let at = world.district_name(p.district).to_string();
    let text =
        format!("{} assaulted {} at a street pitch in {at} ({take} coins taken)", world.name_of(a), world.name_of(o));
    world.push_event(EventKind::Assault, &[a, o], text);
}
