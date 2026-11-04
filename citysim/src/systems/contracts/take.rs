use super::*;

/// Is `viewer` a member of the gang or corp `f`, or `f` itself?
pub(super) fn member(world: &World, viewer: EntityId, f: EntityId) -> bool {
    crate::systems::grudges::member_of(world, viewer, f)
}

/// C11: does `viewer` see record `id` on the board (on demand, never
/// cached)? A brokered record: the Fixer's regulars and members of gangs
/// that networked there or hold (or border) its district; a direct one: the
/// buyer, its edges but Enemies, its gang's members and its corp's staff;
/// a public one (no buyer): everyone; a Guard also Security corps' staff.
/// Nobody sees a record on themselves.
pub fn sees(world: &World, viewer: EntityId, id: ContractId) -> bool {
    let Some(c) = world.contracts.get(&id) else { return false };
    if c.target.id() == viewer {
        return false;
    }
    if c.kind == ContractKind::Guard {
        let security = world
            .corp_of_agent(viewer)
            .and_then(|corp| world.comp::<crate::components::Corp>(corp))
            .is_some_and(|corp| corp.niches.contains(&crate::components::Niche::Security));
        if security {
            return true;
        }
    }
    if let Some(b) = c.broker {
        let Some(k) = world.comp::<Broker>(b) else { return false };
        if k.regulars.contains_key(&viewer) {
            return true;
        }
        let Some(g) = world.gang_of(viewer) else { return false };
        if k.gangs.contains(&g) {
            return true;
        }
        let d = world.district_of_building(b).index();
        let mask = world.district_adjacent.get(d).copied().unwrap_or(0);
        return crate::systems::gang::held_districts(world, g)
            .iter()
            .any(|&(hd, _)| hd.index() == d || mask & (1 << hd.index()) != 0);
    }
    // C31: the on-shift city guards of the buyer's district see a corruption record.
    if c.kind == ContractKind::Guard && guard_viewer(world, c, viewer) {
        return true;
    }
    let Some(buyer) = c.buyer else { return true };
    if viewer == buyer || member(world, viewer, buyer) {
        return true;
    }
    if world.edge(buyer, viewer).is_some_and(|e| e.kind != RelKind::Enemy) {
        return true;
    }
    world.gang_of(buyer).is_some_and(|g| world.gang_of(viewer) == Some(g))
}

/// C11: the live records `viewer` sees (the Board panel, the Contract goal,
/// M18's quest log).
pub fn visible(world: &World, viewer: EntityId) -> Vec<ContractId> {
    world.contracts.iter().filter(|(_, c)| c.is_live()).map(|(&i, _)| i).filter(|&i| sees(world, viewer, i)).collect()
}

// ---------------------------------------------------------------------------
// Matching and acceptance
// ---------------------------------------------------------------------------

/// Does `id` hold a Taken record (as taker or crew)?
pub fn holds_taken(world: &World, id: EntityId) -> bool {
    world.by_party.get(&id).is_some_and(|l| {
        l.iter().any(|cid| {
            world
                .contracts
                .get(cid)
                .is_some_and(|c| c.status == ContractStatus::Taken && (c.taker == Some(id) || c.crew.contains(&id)))
        })
    })
}

/// A free living adult: no Sentence, not cuffed, not leaving.
pub fn free_adult(world: &World, id: EntityId) -> bool {
    crate::systems::law::living(world, id)
        && crate::systems::demography::is_adult(world, id)
        && !world.has::<Sentence>(id)
        && world.comp::<Brain>(id).is_some_and(|b| b.cuffed_by.is_none() && !b.emigrating)
        && !crate::systems::abroad::is_away(world, id)
}

/// C12: may `cand` be offered record `c`: not its target, buyer or placing
/// agent, nor the target's Spouse, Family, Parent or Friend; a free adult
/// (and no guard: a shift is a shift); under `gun_lawfulness` for an
/// illegal kind; holding no Taken record.
pub fn eligible(world: &World, c: &Contract, cand: EntityId) -> bool {
    ineligible(world, c, cand, true).is_none()
}

/// [`eligible`]'s reason `cand` may not take `c` (`None`: he may). `will`:
/// the illegal kind's `gun_lawfulness` term is read (M18 P3: a body the
/// human drives supplies the will and skips it; every other gate is
/// ability or circumstance).
pub fn ineligible(world: &World, c: &Contract, cand: EntityId, will: bool) -> Option<&'static str> {
    let t = c.target.id();
    if cand == t || Some(cand) == c.buyer || Some(cand) == c.agent {
        return Some("a party to it");
    }
    // Round 5 U5 (the under-18 rule): nobody under 18 takes a job or rides
    // on its crew (`squad_crew` and `job_crew` read this through `crew_ok`).
    if !free_adult(world, cand) || crate::systems::ages::is_protected(world, cand) {
        return Some("not free to");
    }
    // Round 8 X2 (`OPEN-247`): a mourner or a man lying low takes no job
    // (20k: a widower took a tag at 18:00 and missed his husband's funeral).
    if crate::systems::grief::holed_up(world, cand) {
        return Some("lying low");
    }
    // C31: a corruption record is a bribe: only a city guard under
    // `corrupt_lawfulness` takes it (plan deviation: the spec lets the usual
    // direct viewers see it, and its buyer's friends took it as a week of
    // bodyguarding, walking off their own jobs); no guard takes anything else.
    if is_corruption(world, c) {
        if !corrupt_ok(world, c, cand) {
            return Some("a bribe for the law");
        }
    } else if crate::systems::law::is_guard(world, cand) {
        return Some("a guard on shift");
    }
    // M16b phase 2: kin and friends may march to free their own.
    if c.kind != ContractKind::Rescue
        && world
            .edge(cand, t)
            .is_some_and(|e| matches!(e.kind, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Friend))
    {
        return Some("the mark is your own (kin or a friend)");
    }
    if will && c.kind.illegal() {
        let law = world.comp::<Personality>(cand).map_or(1.0, |p| p.lawfulness);
        if law > world.config.fixers.gun_lawfulness.get(c.kind) {
            return Some("too lawful");
        }
    }
    if holds_taken(world, cand) {
        return Some("already on a job");
    }
    None
}

/// M18 P3 (the board): why `who`, a body the human drives, cannot take
/// record `id` now (`None`: he can). The take's gates of ability and
/// circumstance as an NPC's take reads them (`walk_in_pick`'s own branch,
/// `offer`): he sees it ([`sees`]), it is open and not held for another
/// walk-in, [`ineligible`] (the lawfulness term waived), the gun gate's
/// skill floor (`gun_skill_min`, a brokered record or a violent direct
/// one), the stakeout's reach ([`hunt_in_reach`]), a brokered record's
/// Fixer open. Waived (the human's will): mode A (`accept_min`: the price
/// against the risk), the gun gate's day-wage term, `gun_lawfulness`. No
/// minor is ever a mark.
pub fn player_take_refusal(world: &World, who: EntityId, id: ContractId) -> Option<String> {
    let Some(c) = world.contracts.get(&id) else { return Some("gone".into()) };
    if c.status != ContractStatus::Open {
        return Some(c.status.label().to_string());
    }
    if !sees(world, who, id) {
        return Some("not on a board you know".into());
    }
    if world.contract_runs.contains_key(&who) || world.mission_of.contains_key(&who) {
        return Some("already on a job".into());
    }
    if !matches!(c.kind, ContractKind::Guard | ContractKind::Escort)
        && c.target_agent().is_some_and(|t| crate::systems::ages::is_protected(world, t))
    {
        return Some("a child is never a mark".into());
    }
    if held_for_other(world, c, who) {
        return Some("held for someone on his way".into());
    }
    // a brokered Locate pays its sightings from escrow: spent, the take pays
    // nothing (`spent_locate`)
    if c.kind == ContractKind::Locate
        && c.brokered()
        && (c.escrow < world.config.bounty.per_sighting || c.paid_sightings >= world.config.bounty.cap_sightings)
    {
        return Some("spent: nothing left to pay a sighting".into());
    }
    if mark_gone_past_deadline(world, c) {
        if let Some((why, _)) = c.target_agent().and_then(|t| mark_away(world, t)) {
            return Some(why);
        }
    }
    if let Some(why) = ineligible(world, c, who, false) {
        return Some(why.into());
    }
    if c.broker.is_some() || (c.kind.violent() && direct_gated(world, c.kind)) {
        let best = world.comp::<Skills>(who).map_or(0.0, |s| {
            crate::systems::combat::skill_power(world, s)
                .max(s.stealth)
                .max(s.hacking)
                .max(s.persuasion)
                .max(s.intimidation)
        });
        if best < world.config.fixers.gun_skill_min {
            return Some(format!("not the skills for it ({best:.2} < {:.2})", world.config.fixers.gun_skill_min));
        }
    }
    if !hunt_in_reach(world, c, who) {
        return Some("too far from the mark's haunts".into());
    }
    if let Some(f) = c.broker {
        if !fixer_open(world, f) {
            return Some(format!("{} is shut", world.name_of(f)));
        }
        if crate::systems::service::desk_server(world, f, who).is_none() {
            return Some(format!("nobody at the desk of {} now", world.name_of(f)));
        }
    }
    None
}

/// M18 P3 (the board's line): why the mark cannot be walked to now
/// (`mark_away`: the cells, dead, abroad), if so.
pub fn mark_note(world: &World, c: &Contract) -> Option<String> {
    c.target_agent().and_then(|t| mark_away(world, t)).map(|(why, _)| why)
}

/// M18 P3: `who` (a body the human drives) takes record `id`: at its
/// Fixer's desk in person for a brokered one, else from its buyer's hand;
/// the crew as an NPC's take brings one (`squad_crew`). `false`: refused
/// (the reason to the event ring).
pub fn player_take(world: &mut World, who: EntityId, id: ContractId) -> bool {
    if let Some(why) = player_take_refusal(world, who, id) {
        let text = format!("{} could not take the job: {why}", world.name_of(who));
        world.push_event(EventKind::Contract, &[who], text);
        return false;
    }
    let Some(c) = world.contracts.get(&id).cloned() else { return false };
    let crew = squad_crew(world, &c, who);
    // in person at a Fixer's desk; a direct record is the buyer's word
    let walker = c.broker.map(|_| who);
    accept_at(world, id, who, &crew, walker)
}

/// M18 P3: the open record at Fixer `f` that `who` claimed (walking to its
/// desk to take it, `player::act`), if any.
pub(super) fn claimed_at(world: &World, who: EntityId, f: EntityId) -> Option<ContractId> {
    let k = world.comp::<Broker>(f)?;
    k.book.iter().copied().find(|id| {
        world
            .contracts
            .get(id)
            .is_some_and(|c| c.status == ContractStatus::Open && c.claimed.is_some_and(|(w, _)| w == who))
    })
}

/// The pressure pass F1: a record run as a hunt (`plan`'s `GoTo(Intel)`:
/// an agent target, no Guard or Escort, no crew's Rescue) is offered only
/// (`offer`, a gun's own acceptance; not a gang's crew, which marches as a
/// mission) to a candidate within `[distance.reach] stakeout_ticks` of the target's
/// Home or workplace door (the habits a stakeout watches). Phase 4 cools a
/// stakeout past the reach, so a far taker held the record to its deadline
/// (20k: contracts fulfilled 71 -> 45). `true` off, or with no door known.
pub(super) fn hunt_in_reach(world: &World, c: &Contract, cand: EntityId) -> bool {
    let Some(cap) = crate::systems::distance::reach(world).map(|r| r.stakeout_ticks) else { return true };
    let Target::Agent(t) = c.target else { return true };
    if matches!(c.kind, ContractKind::Guard | ContractKind::Escort | ContractKind::Rescue) {
        return true;
    }
    let Some(from) = crate::systems::life::origin(world, cand) else { return true };
    let door = |b: Option<EntityId>| b.and_then(|b| world.comp::<Building>(b)).map(|bd| bd.door);
    let doors = [home_door(world, t), door(world.comp::<Job>(t).and_then(|j| j.employer))];
    let mut known = false;
    for d in doors.into_iter().flatten() {
        known = true;
        if crate::systems::distance::travel(world, from, d) <= cap {
            return true;
        }
    }
    !known
}
