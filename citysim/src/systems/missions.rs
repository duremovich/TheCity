//! M16a § 3 (plan C22-C25): missions and the strike estimate.
//!
//! A game abstraction: a mission is a crew list marching to a door, and its
//! outcome is the raid machinery's seeded dice (`raid::fight_out`); the
//! strike estimate is a number, `p_win = logistic(strike_k × (S_crew ÷
//! S_def − 1))` over summed `raid::strength` scores. Phase 1 lands the
//! estimate alone (`estimate`, `expected_defenders`, `MEDIAN`), which the
//! quote, `risk_for` and the ledger read; phase 2 fills the rest: the
//! strike decision (`strike`, `decide`: a deterministic score, no draw),
//! its political term and the `Trespass` shocks, the mission's scripted
//! plan and step checks (`plan`, `can_start`), the departure, the wait at
//! the door, the defenders and the brawl (`depart`, `wait_for_crew`,
//! `defenders`, `brawl`), and the per-tick validity pass (`check`). The
//! mission's record-side functions (`open_mission`, `squad_crew`,
//! `sell_out`, the gang taker) live in `systems::contracts`.

use crate::components::{
    ActionInstance, Brain, Building, Controller, CorpShock, GoalKind, Household, Position, RelKind, Sentence, Shock,
    TilePos,
};
use crate::config::MissionsCfg;
use crate::contract::{ContractId, ContractKind, ContractStatus, Decision, StrikeTerms, Target};
use crate::entity::EntityId;
use crate::events::EventKind;
use crate::goap::{ActionKind, LocationKey, Plan};
use crate::systems::{contracts, faction, law, raid};
use crate::time::{Tick, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::word::{Deed, Regard};
use crate::world::World;

/// C7: the phantom median gun of the quote (fighting 0.5, courage 0.5, no
/// Kit: strength 0.625); `strength_of(MEDIAN)` reads it.
pub const MEDIAN: EntityId = EntityId::NONE;

/// The median gun's strength (`0.5 + 0.25 × 0.5`).
pub const MEDIAN_STRENGTH: f32 = 0.625;

/// `raid::strength`, with the phantom median gun.
pub fn strength_of(world: &World, id: EntityId) -> f32 {
    if id == MEDIAN {
        MEDIAN_STRENGTH
    } else {
        crate::systems::raid::strength(world, id)
    }
}

/// `1 / (1 + e^−x)`.
pub fn logistic(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// C24: `p_win` from the two summed strengths.
pub fn p_win(world: &World, s_crew: f32, s_def: f32) -> f32 {
    logistic(world.config.missions.strike_k * (s_crew / s_def.max(0.1) - 1.0))
}

/// C23: the defenders a strike on `target` expects: an agent target and
/// up to `ally_cap_n` of its gang members or Friends with courage ≥ 0.5
/// living in its Home district, strongest first (ties the lower id); a
/// building target, its private and posted guards (none at HEAD's phase 1:
/// an empty list, so a Guard or Locate on a building reads its risk flat).
pub fn expected_defenders(world: &World, target: &Target) -> Vec<EntityId> {
    let Target::Agent(t) = *target else { return Vec::new() };
    if !crate::systems::law::living(world, t) {
        return Vec::new();
    }
    let home_d = world.comp::<Household>(t).and_then(|h| h.home).map(|h| world.district_of_building(h));
    let mut allies: Vec<EntityId> = Vec::new();
    if let Some(g) = world.gang_of(t).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        allies.extend(g.members.iter().copied().filter(|&m| m != t));
    }
    for o in world.neighbours(t) {
        if world.edge(t, o).is_some_and(|e| e.kind == RelKind::Friend) && !allies.contains(&o) {
            allies.push(o);
        }
    }
    let mut allies: Vec<(f32, EntityId)> = allies
        .into_iter()
        .filter(|&a| crate::systems::law::living(world, a) && crate::systems::law::courage(world, a) >= 0.5)
        .filter(|&a| !world.has::<crate::components::Sentence>(a))
        .filter(|&a| {
            home_d.is_some()
                && world.comp::<Household>(a).and_then(|h| h.home).map(|h| world.district_of_building(h)) == home_d
        })
        .map(|a| (strength_of(world, a), a))
        .collect();
    allies.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let cap = usize::from(world.config.missions.ally_cap_n);
    let mut out = vec![t];
    out.extend(allies.into_iter().take(cap).map(|(_, a)| a));
    out
}

/// C23: the strike's `p_win` for `crew` against `target`'s expected
/// defenders (the quote's, `risk_for`'s and the ledger's estimate). An
/// empty defender list (a building with no guards) reads `S_def` 0.1.
pub fn estimate(world: &World, crew: &[EntityId], target: &Target) -> f32 {
    let s_crew: f32 = crew.iter().map(|&c| strength_of(world, c)).sum();
    let s_def: f32 = expected_defenders(world, target).iter().map(|&d| strength_of(world, d)).sum();
    p_win(world, s_crew, s_def)
}

/// `estimate` against a defender strength already summed (matching scores
/// many candidates against one target).
pub fn estimate_against(world: &World, crew: &[EntityId], s_def: f32) -> f32 {
    let s_crew: f32 = crew.iter().map(|&c| strength_of(world, c)).sum();
    p_win(world, s_crew, s_def)
}

/// The summed strength of `target`'s expected defenders.
pub fn defender_strength(world: &World, target: &Target) -> f32 {
    expected_defenders(world, target).iter().map(|&d| strength_of(world, d)).sum()
}

// ---------------------------------------------------------------------------
// The strike decision (C24)
// ---------------------------------------------------------------------------

/// C24's numbers, pure (the Strike table's unit test reads it): `p_win =
/// logistic(strike_k × (S_crew ÷ max(S_def, 0.1) − 1))` over strengths
/// already multiplied by the defenders' alertness; `political = pol_w ×
/// control_share × (0.5 + 0.5 × fear) × (1 + max(0, value))`, doubled for a
/// target who is a member of the controller (`pol` is `None` when the
/// controller is the City, Contested, the buyer's faction or the taker's
/// gang); `ev = p_win − (1 − p_win) × loss_w − political`. Returns
/// `(p_win, political, ev)`.
pub fn strike_numbers(cfg: &MissionsCfg, s_crew: f32, s_def: f32, pol: Option<(f32, Regard, bool)>) -> (f32, f32, f32) {
    let p = logistic(cfg.strike_k * (s_crew / s_def.max(0.1) - 1.0));
    let political = pol.map_or(0.0, |(share, r, member)| {
        let x = cfg.pol_w * share * (0.5 + 0.5 * r.fear) * (1.0 + r.value.max(0.0));
        if member {
            2.0 * x
        } else {
            x
        }
    });
    let ev = p - (1.0 - p) * cfg.loss_w - political;
    (p, political, ev)
}

/// C24: the buyer's faction: a gang or corp buyer itself, else the buyer
/// agent's (or the placing agent's) gang, else the Law (the Jail building
/// that carries `Law`, `reputation::factions`' id); `EntityId::NONE` in a
/// city with no Jail.
pub fn buyer_faction(world: &World, c: &crate::contract::Contract) -> EntityId {
    if let Some(b) =
        c.buyer.filter(|&b| world.has::<crate::components::Gang>(b) || world.has::<crate::components::Corp>(b))
    {
        return b;
    }
    if let Some(g) = c.buyer.or(c.agent).and_then(|a| world.gang_of(a)) {
        return g;
    }
    world
        .building_of_kind(crate::components::BuildingKind::Jail)
        .filter(|&j| world.has::<crate::components::Law>(j))
        .unwrap_or(EntityId::NONE)
}

/// The taker's gang: a gang taker itself, else the agent taker's gang.
pub fn taker_gang(world: &World, c: &crate::contract::Contract) -> Option<EntityId> {
    let t = c.taker?;
    if world.has::<crate::components::Gang>(t) {
        Some(t)
    } else {
        world.gang_of(t)
    }
}

/// The faction entity behind a controller (`None` for the City and
/// Contested: the City's law reads `alertness_mult(None)`).
pub fn controller_faction(c: Controller) -> Option<EntityId> {
    match c {
        Controller::Gang(g) => Some(g),
        Controller::Corp(c) => Some(c),
        Controller::City | Controller::Contested => None,
    }
}

/// A foreign controller of the door's district: a gang or corp that is
/// neither the buyer's faction nor the taker's gang (the political term
/// and the `Trespass` shock apply).
fn foreign_controller(world: &World, c: &crate::contract::Contract, door: TilePos) -> Option<(EntityId, f32)> {
    let d = world.district(world.district_of(door));
    let f = controller_faction(d.control)?;
    let bf = buyer_faction(world, c);
    (f != bf && Some(f) != taker_gang(world, c)).then_some((f, d.control_share))
}

/// C24 (and 2.4, the attention hook): the strike terms for `crew` at
/// `door` on record `id`'s agent target, against its expected defenders
/// (C23's estimate; the crew excluded) times `faction::alertness_mult` of
/// the district's controller (`None` for the City or Contested), plus the
/// Guard takers standing over it times its own faction's. The decision:
/// Strike at `ev ≥ 0`; else Hold while `holds < hold_days`; else SellOut
/// to a controlling gang that would take it (`contracts::can_sell_out`);
/// else Fail. Pure: no draw, no write. `None` without an agent target.
pub fn strike(world: &World, id: ContractId, crew: &[EntityId], door: TilePos) -> Option<StrikeTerms> {
    let c = world.contracts.get(&id)?;
    let t = c.target_agent()?;
    let cfg = &world.config.missions;
    let dist = world.district(world.district_of(door));
    let control = dist.control;
    let mut defs = expected_defenders(world, &c.target);
    defs.retain(|x| !crew.contains(x));
    let s_def0: f32 = defs.iter().map(|&x| strength_of(world, x)).sum();
    let guards: f32 = world.contract_guards.get(&t).map_or(0.0, |l| {
        l.iter()
            .copied()
            .filter(|g| !crew.contains(g) && !defs.contains(g) && law::living(world, *g))
            .map(|g| strength_of(world, g))
            .sum()
    });
    let client_faction = world.gang_of(t).or_else(|| world.corp_of_agent(t));
    let s_def = s_def0 * faction::alertness_mult(world, controller_faction(control))
        + guards * faction::alertness_mult(world, client_faction);
    let s_crew: f32 = crew.iter().map(|&x| strength_of(world, x)).sum();
    let bf = buyer_faction(world, c);
    let pol = foreign_controller(world, c, door).map(|(f, share)| {
        (share, crate::systems::reputation::regard(world, f, bf), crate::systems::grudges::member_of(world, t, f))
    });
    let (p_win, political, ev) = strike_numbers(cfg, s_crew, s_def, pol);
    let decision = if ev >= 0.0 {
        Decision::Strike
    } else if c.holds < cfg.hold_days {
        Decision::Hold
    } else if matches!(control, Controller::Gang(g) if contracts::can_sell_out(world, id, g)) {
        Decision::SellOut
    } else {
        Decision::Fail
    };
    Some(StrikeTerms { p_win, controller: control.csv_code(), political, ev, decision })
}

/// C24: decide and act. The terms are recorded on the record (the Mission
/// panel's); a Strike in a foreign controller's district pushes
/// `Shock::Trespass` (a gang) or `CorpShock::Trespass` (a corp) and posts a
/// `Raided` deed (actor the buyer's faction, object the controller) into
/// the district's pool; a Hold counts (`holds + 1`; the caller moves the
/// muster, the due tick or the run a day); a SellOut hands the record to
/// the district's gang (`contracts::sell_out`; a short buyer lapses the
/// offer and the attempt fails); a Fail is `StrikeDeclined`
/// (`strikes_declined_pol` when the political term decided it) and a failed
/// attempt. Returns the decision acted on.
pub fn decide(world: &mut World, id: ContractId, crew: &[EntityId], door: TilePos) -> Decision {
    let Some(terms) = strike(world, id, crew, door) else { return Decision::Fail };
    if let Some(c) = world.contracts.get_mut(&id) {
        c.strike = Some(terms);
        c.political = terms.political;
    }
    match terms.decision {
        Decision::Strike => {
            trespass(world, id, door);
            Decision::Strike
        }
        Decision::Hold => {
            if let Some(c) = world.contracts.get_mut(&id) {
                c.holds = c.holds.saturating_add(1);
            }
            Decision::Hold
        }
        Decision::SellOut => {
            let control = world.district(world.district_of(door)).control;
            let sold = match control {
                Controller::Gang(g) => contracts::sell_out(world, id, g),
                _ => false,
            };
            if sold {
                Decision::SellOut
            } else {
                contracts::fail_attempt(world, id, "the sell-out lapsed");
                Decision::Fail
            }
        }
        Decision::Fail => {
            let Some(c) = world.contracts.get(&id).cloned() else { return Decision::Fail };
            if terms.political > 0.0 && terms.ev + terms.political >= 0.0 {
                world.stats.current.contract.strikes_declined_pol += 1;
            }
            let taker = c.taker.unwrap_or(EntityId::NONE);
            let t = c.target.id();
            let district = world.district_name(world.district_of(door)).to_string();
            let text = format!(
                "{} called off the strike on {} in {district} (cost {:.2})",
                world.owner_label(c.taker),
                world.name_of(t),
                terms.political
            );
            world.push_event(EventKind::StrikeDeclined, &[taker, t], text.clone());
            contracts::note(world, id, text);
            contracts::fail_attempt(world, id, "the strike was called off");
            Decision::Fail
        }
    }
}

/// C24: a Strike in a district whose controller is another faction.
fn trespass(world: &mut World, id: ContractId, door: TilePos) {
    let Some(c) = world.contracts.get(&id).cloned() else { return };
    let Some((f, _)) = foreign_controller(world, &c, door) else { return };
    let bf = buyer_faction(world, &c);
    let by = (bf != EntityId::NONE).then_some(bf);
    if world.has::<crate::components::Gang>(f) {
        crate::systems::gang::push_shock(world, f, Shock::Trespass { by });
    } else if world.has::<crate::components::Corp>(f) {
        crate::systems::ownership::push_corp_shock(world, f, CorpShock::Trespass);
    }
    let d = world.district_of(door);
    crate::systems::gossip::post_deed(world, d, Deed::Raided, by, Some(f));
}

// ---------------------------------------------------------------------------
// The mission's march (C22, C23)
// ---------------------------------------------------------------------------

/// The mission an agent marches with, and its record, while that record is
/// Taken.
fn mission_for(world: &World, agent: EntityId) -> Option<(ContractId, &crate::contract::Mission)> {
    if world.missions.is_empty() {
        return None;
    }
    let id = *world.mission_of.get(&agent)?;
    let m = world.missions.get(&id)?;
    world.contracts.get(&id).filter(|c| c.status == ContractStatus::Taken)?;
    Some((id, m))
}

/// The strike was decided Strike for this mission and its hour has come:
/// the crew has left the muster.
fn departed(world: &World, id: ContractId, m: &crate::contract::Mission) -> bool {
    world.tick >= m.raid_at
        && world.contracts.get(&id).and_then(|c| c.strike).is_some_and(|s| s.decision == Decision::Strike)
}

/// C22: a crew member's scripted plan under `GoalKind::Contract`:
/// `GoTo(MusterPoint) → Muster → GoTo(RaidTarget) → Brawl`, the muster
/// dropped once the crew has left it (a latecomer goes straight to the
/// door). The raid's actions and location keys (`raid::muster_point` and
/// `raid::target_tile` read the mission). `None` cools the goal.
pub fn plan(world: &World, id: EntityId) -> Option<Plan> {
    let (cid, m) = mission_for(world, id)?;
    let t = world.contracts.get(&cid)?.target_agent()?;
    let step = |action| ActionInstance { action, target: None, tile: None };
    let mut steps = Vec::new();
    if !departed(world, cid, m) {
        steps.push(step(ActionKind::GoTo(LocationKey::MusterPoint)));
        steps.push(step(ActionKind::Muster));
    }
    steps.push(step(ActionKind::GoTo(LocationKey::RaidTarget)));
    steps.push(step(ActionKind::Brawl));
    Some(Plan { goal: GoalKind::Contract, target: Some(t), steps, started_tick: world.tick })
}

/// The mission steps' start check (`contracts::can_start`): `Muster` at the
/// muster tile (within a tile), `Brawl` within `raid_gather_radius` of the
/// door once the crew has left the muster; the record Taken.
pub fn can_start(world: &World, id: EntityId, kind: ActionKind) -> bool {
    let Some((cid, m)) = mission_for(world, id) else { return false };
    let Some(p) = world.comp::<Position>(id) else { return false };
    match kind {
        ActionKind::Muster => law::chebyshev(p.tile, m.muster) <= 1,
        ActionKind::Brawl => {
            departed(world, cid, m) && law::chebyshev(p.tile, m.door) <= world.config.gangs.raid_gather_radius
        }
        _ => false,
    }
}

/// C22, `raid::depart`'s Mission arm (a crew member's `Muster` completes):
/// before `raid_at`, nothing; once the strike is decided Strike, every
/// member follows; else the first at the muster decides (C24): a Hold moves
/// `raid_at` a day, re-reads the door and sends the crew about its day
/// until three hours before; a SellOut or a Fail ends this mission.
pub fn depart(world: &mut World, _actor: EntityId, id: ContractId) -> bool {
    let now = world.tick;
    let Some(m) = world.missions.get(&id).cloned() else { return false };
    if world.contracts.get(&id).is_none_or(|c| c.status != ContractStatus::Taken) {
        return false;
    }
    if now < m.raid_at {
        return false;
    }
    if departed(world, id, &m) {
        return true;
    }
    match decide(world, id, &m.crew, m.door) {
        Decision::Strike => true,
        Decision::Hold => {
            let raid_at = m.raid_at + TICKS_PER_DAY;
            let (door, door_building) = contracts::mission_door(world, id, raid_at);
            if let Some(x) = world.missions.get_mut(&id) {
                x.raid_at = raid_at;
                x.door = door;
                x.door_building = door_building;
            }
            let rest = raid_at.saturating_sub(3 * TICKS_PER_HOUR);
            for &a in &m.crew {
                if let Some(b) = world.comp_mut::<Brain>(a) {
                    b.cooldowns.insert(GoalKind::Contract, rest);
                }
            }
            false
        }
        Decision::SellOut | Decision::Fail => false,
    }
}

/// Is `a` at the mission's door: inside its building, or within
/// `raid_gather_radius` of the door tile?
fn at_door(world: &World, m: &crate::contract::Mission, a: EntityId) -> bool {
    let r = world.config.gangs.raid_gather_radius;
    world.comp::<Position>(a).is_some_and(|p| {
        (m.door_building.is_some() && p.building == m.door_building) || law::chebyshev(p.tile, m.door) <= r
    })
}

/// C22, `raid::wait_for_crew`'s Mission arm (the gang arm's rule over the
/// crew, goal `Contract`): the first at the door holds the Brawl while
/// fewer than `min(3, crew, marching)` of the crew stand there, or the
/// target is not there (C23), until the last hour of the march window.
pub fn wait_for_crew(world: &World, actor: EntityId, id: ContractId) -> bool {
    let Some(m) = world.missions.get(&id) else { return false };
    let now = world.tick;
    if now + TICKS_PER_HOUR >= m.raid_at + faction::RAID_MARCH_TICKS {
        return false;
    }
    let marching = m
        .crew
        .iter()
        .filter(|&&a| {
            law::living(world, a)
                && !world.has::<Sentence>(a)
                && world.comp::<Brain>(a).is_some_and(|b| b.current_goal == Some(GoalKind::Contract))
        })
        .count();
    let present = raid::gathered_for(world, &m.crew, actor, m.door, GoalKind::Contract).len();
    if present < m.crew.len().min(3).min(marching) {
        return true;
    }
    let t = world.contracts.get(&id).and_then(|c| c.target_agent());
    !t.is_some_and(|t| law::living(world, t) && at_door(world, m, t))
}

/// C23: the defenders at a mission's door: the target when there, the
/// Guard takers standing over it, its gang's members and its Friends at the
/// door with courage ≥ 0.5 (free, not of the crew); at a building door its
/// posted private guards (`raid::posted_guards`), the powered robots first
/// (`raid::robots_first`). Strongest first.
pub fn defenders(world: &World, id: ContractId) -> Vec<EntityId> {
    let Some(m) = world.missions.get(&id) else { return Vec::new() };
    let Some(t) = world.contracts.get(&id).and_then(|c| c.target_agent()) else { return Vec::new() };
    let fit = |a: EntityId| law::living(world, a) && !world.has::<Sentence>(a) && !m.crew.contains(&a);
    let mut out: Vec<EntityId> = Vec::new();
    if fit(t) && at_door(world, m, t) {
        out.push(t);
    }
    if let Some(l) = world.contract_guards.get(&t) {
        out.extend(l.iter().copied().filter(|&g| fit(g) && at_door(world, m, g)));
    }
    let mut allies: Vec<EntityId> = Vec::new();
    if let Some(g) = world.gang_of(t).and_then(|g| world.comp::<crate::components::Gang>(g)) {
        allies.extend(g.members.iter().copied());
    }
    allies.extend(world.neighbours(t).filter(|&o| world.edge(t, o).is_some_and(|e| e.kind == RelKind::Friend)));
    for a in allies {
        if a != t && fit(a) && law::courage(world, a) >= 0.5 && at_door(world, m, a) {
            out.push(a);
        }
    }
    if let Some(b) = m.door_building {
        out.extend(raid::posted_guards(world, b).into_iter().filter(|&g| fit(g)));
    }
    out.sort_unstable();
    out.dedup();
    raid::by_strength(world, &mut out);
    if let Some(b) = m.door_building {
        raid::robots_first(world, b, &mut out);
    }
    out
}

/// The brawl at a door between a crew and its defenders: the raid
/// machinery's `fight_out` (crossfire, litter, the pairings on the world
/// stream), with the arguments a gang raid passes (`kill_mult` 1.0, no
/// riot). `test_mission_fight_out_matches_gang_raid` holds the two paths
/// together.
pub fn fight(
    world: &mut World,
    raiders: &mut Vec<EntityId>,
    defenders: &mut Vec<EntityId>,
    door: TilePos,
    place: &str,
) -> raid::Tally {
    raid::fight_out(world, raiders, defenders, door, place, 1.0, None)
}

/// C23, `raid::resolve`'s Mission arm (the first at the door, after
/// `wait_for_crew`): the target absent fails the attempt; else the crew
/// present against the defenders ([`defenders`]) through [`fight`]; a win
/// (nobody left standing) on a Hit whose target still lives gives the
/// strongest raider standing one more strike at `hunt_kill_p`
/// (`law::resolve_fight_mods`, a Murder or an Assault as `Attack`'s). A
/// Hit's target dead by the crew's hands was settled by `contracts::on_death`;
/// a Beat won is fulfilled; anything else fails the attempt.
pub fn brawl(world: &mut World, actor: EntityId) -> Option<raid::Outcome> {
    let (id, m) = mission_for(world, actor).map(|(i, m)| (i, m.clone()))?;
    let c = world.contracts.get(&id)?.clone();
    let t = c.target_agent()?;
    let place = match m.door_building {
        Some(b) => world.name_of(b),
        None => format!("a street in {}", world.district_name(world.district_of(m.door))),
    };
    let crew_name = format!("{}'s crew", world.owner_label(c.taker));
    let tname = world.name_of(t);
    let mut raiders = raid::gathered_for(world, &m.crew, actor, m.door, GoalKind::Contract);
    let mut defs = defenders(world, id);
    if !defs.contains(&t) {
        let text = format!("{crew_name} found no {tname} at {place}");
        contracts::fail_attempt(world, id, &text);
        return Some(raid::Outcome::Lost);
    }
    let (n_raiders, n_defenders) = (raiders.len(), defs.len());
    let tally = fight(world, &mut raiders, &mut defs, m.door, &place);
    let won = defs.is_empty();
    let mut finished = false;
    if won && c.kind == ContractKind::Hit && law::living(world, t) && taken(world, id) {
        let mut standing: Vec<EntityId> =
            raiders.iter().copied().filter(|&r| law::living(world, r) && !world.has::<Sentence>(r)).collect();
        raid::by_strength(world, &mut standing);
        if let Some(&r) = standing.first() {
            let mods = law::FightMods { kill_p: Some(world.config.hunt.hunt_kill_p), ..Default::default() };
            let (_, loser, died) = law::resolve_fight_mods(world, r, t, mods);
            let murder = died && loser == t;
            let crime = if murder { crate::components::Crime::Murder } else { crate::components::Crime::Assault };
            let kind = if murder { EventKind::Murder } else { EventKind::Assault };
            let fell = if died && loser == r { " and died" } else { "" };
            let text = format!("{} attacked {tname} at {place}{fell}", world.name_of(r));
            world.push_event(kind, &[r, t], text);
            if law::living(world, r) {
                law::raise_crime_on(world, r, (!murder).then_some(t), Some(t), crime, m.door);
            }
            finished = murder;
        }
    }
    let result = if won { "won" } else { "lost" };
    let line =
        format!("{crew_name} hit {tname} at {place}: {result} ({n_raiders} vs {n_defenders}, {} dead)", tally.deaths);
    contracts::note(world, id, line.clone());
    if taken(world, id) {
        match (won, c.kind) {
            (true, ContractKind::Beat) => contracts::settle(world, id, contracts::Settle::Fulfilled, &line),
            (true, ContractKind::Hit) if !finished && law::living(world, t) => {
                contracts::fail_attempt(world, id, &format!("{line}; {tname} survived"))
            }
            _ => contracts::fail_attempt(world, id, &line),
        }
    }
    Some(if won { raid::Outcome::Won } else { raid::Outcome::Lost })
}

fn taken(world: &World, id: ContractId) -> bool {
    world.contracts.get(&id).is_some_and(|c| c.status == ContractStatus::Taken)
}

/// The per-tick validity pass over the live missions (C36, O(missions ×
/// crew)): a record no longer Taken ends its mission; a crew member dead,
/// jailed, cuffed or leaving drops out (`contracts::drop_crew`; the last
/// one out fails the attempt); a mission still standing when its march
/// window has closed fails the attempt.
pub fn check(world: &mut World) {
    if world.missions.is_empty() {
        return;
    }
    let now = world.tick;
    let ids: Vec<ContractId> = world.missions.keys().copied().collect();
    for id in ids {
        if !taken(world, id) {
            contracts::end_mission(world, id);
            continue;
        }
        let Some(m) = world.missions.get(&id).cloned() else { continue };
        for &a in &m.crew {
            if !contracts::free_adult(world, a) {
                contracts::drop_crew(world, id, a);
            }
        }
        let Some(m) = world.missions.get(&id) else { continue };
        if now >= m.raid_at + faction::RAID_MARCH_TICKS {
            contracts::fail_attempt(world, id, "the crew never reached the door");
        }
    }
}

/// A Hold's day (C24) for the solo strike and the ledger: the next look.
pub fn hold_until(now: Tick) -> Tick {
    now + TICKS_PER_DAY
}

/// The muster's tile a building gives (outside its door).
pub fn door_of(world: &World, b: EntityId) -> Option<TilePos> {
    world.comp::<Building>(b).map(|bd| world.outside_door(bd))
}

/// `Household` re-export for the contracts side's muster rule.
pub fn home_of(world: &World, a: EntityId) -> Option<EntityId> {
    world.comp::<Household>(a).and_then(|h| h.home)
}
