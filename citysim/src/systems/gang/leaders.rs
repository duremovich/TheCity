use super::*;

/// The leader ranking: free members with a Brain by `loyalty +
/// days_in_gang / 100`, best first (ties the lower id). M12 D36: the
/// runner-up is the lieutenant.
pub fn leader_ranking(world: &World, gang: EntityId) -> Vec<EntityId> {
    leader_scores(world, gang).into_iter().map(|(m, _)| m).collect()
}

/// A member's days in the gang.
fn gang_days(world: &World, m: EntityId) -> f32 {
    world.comp::<GangMember>(m).map_or(0.0, |g| world.tick.saturating_sub(g.joined_tick) as f32 / TICKS_PER_DAY as f32)
}

/// Shadow V4 bug 8: a member of `[gangs] leader_min_days` or more (the
/// leader ranking's first tenure class).
pub fn leader_veteran(world: &World, m: EntityId) -> bool {
    gang_days(world, m) >= world.config.gangs.leader_min_days as f32
}

/// [`leader_ranking`] with each member's score, best first.
pub fn leader_scores(world: &World, gang: EntityId) -> Vec<(EntityId, f32)> {
    let Some(g) = world.comp::<Gang>(gang) else { return Vec::new() };
    // Lives at work phase 6 (§ 4.5, the ladder): + raids won as the head × `raid_w`.
    let ladder = crate::systems::careers::lieutenants_on(world);
    let raid_w = world.config.gangs.raid_w;
    let mut ranked: Vec<(EntityId, f32, bool)> = g
        .members
        .iter()
        .copied()
        .filter(|&m| world.has::<Brain>(m) && !world.has::<Sentence>(m))
        .map(|m| {
            let loyalty = world.comp::<Personality>(m).map_or(0.0, |p| p.loyalty);
            let mut score = loyalty + gang_days(world, m) / 100.0;
            if ladder {
                score += raid_w * f32::from(world.comp::<GangMember>(m).map_or(0, |x| x.raids_won));
            }
            // The pressure pass phase 5b: duty kept and time served (0 off).
            score += crate::systems::crew::standing_score(world, m);
            (m, score, leader_veteran(world, m))
        })
        .collect();
    // Shadow V4 bug 8: a member of `[gangs] leader_min_days` or more ranks
    // above a newer one (a clinic owner led the Choir 3 days after joining
    // on loyalty alone).
    ranked.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.total_cmp(&a.1)).then(a.0.cmp(&b.0)));
    ranked.into_iter().map(|(m, s, _)| (m, s)).collect()
}

/// `leader = argmax(loyalty + days_in_gang / 100)` over members with a Brain
/// and no Sentence; rank 2, others 0. A change of leader is a shock.
pub fn recompute_leader(world: &mut World, gang: EntityId) {
    recompute_leader_after(world, gang, None);
}

/// [`recompute_leader`] after `fallen` was killed or jailed: when the fallen
/// was the leader and a new one takes over, the gang is due a split check
/// (M12 D36, `Gang.split_check`).
fn recompute_leader_after(world: &mut World, gang: EntityId, fallen: Option<EntityId>) {
    // M17 phase 4b: a claimant killed or jailed loses the civil war at once;
    // the winner is crowned and no new succession follows.
    let war_lost = fallen.is_some_and(|f| crate::systems::succession::claimant_fell(world, gang, f));
    recompute_leader_inner(world, gang, fallen, war_lost);
}

/// [`recompute_leader`] at a civil war's end (`succession::end`): the war is
/// lost by the claimant, so the loser is nobody's jailed boss and no split
/// check follows (phase-8 review: the old after-the-fact boss patch).
pub fn recompute_leader_after_war(world: &mut World, gang: EntityId) {
    recompute_leader_inner(world, gang, None, true);
}

fn recompute_leader_inner(world: &mut World, gang: EntityId, fallen: Option<EntityId>, war_lost: bool) {
    recompute_leader_back(world, gang, fallen, war_lost, None);
}

/// [`recompute_leader_inner`], `back` the boss just out of the cells (he
/// walks back into the chair).
fn recompute_leader_back(
    world: &mut World,
    gang: EntityId,
    fallen: Option<EntityId>,
    war_lost: bool,
    back: Option<EntityId>,
) {
    let Some(g) = world.comp::<Gang>(gang) else { return };
    let (members, old, crowned, at_war) = (g.members.clone(), g.leader, g.crowned, g.war.is_some());
    let scores = leader_scores(world, gang);
    let ranking: Vec<EntityId> = scores.iter().map(|x| x.0).collect();
    // The war's heir, then its winner (or a splinter's founder), leads while
    // a free member. M17 review: the crown stays while its holder lives in
    // the gang (a night in the cells leaves an acting leader, not a new king).
    let kept = crowned.filter(|&c| crate::systems::law::living(world, c) && world.gang_of(c) == Some(gang));
    let crowned = kept.filter(|c| ranking.contains(c));
    // Round 8 X4 (`OPEN-241`): the boss out of the cells walks back into the
    // chair; the sitting leader, free in the gang, keeps it until a member of
    // a higher tenure class, or of his and `leader_keep_margin` ahead on the
    // ranking score, passes him.
    let margin = world.config.gangs.leader_keep_margin;
    let score = |m: EntityId| scores.iter().find(|x| x.0 == m).map_or(0.0, |x| x.1);
    let top = ranking.first().copied();
    let sitting = old.filter(|o| margin > 0.0 && ranking.contains(o)).map(|o| match top {
        Some(t)
            if t != o && ((leader_veteran(world, t) && !leader_veteran(world, o)) || score(t) - score(o) > margin) =>
        {
            t
        }
        _ => o,
    });
    let leader = crowned.or_else(|| back.filter(|b| ranking.contains(b))).or(sitting).or(top);
    // Lives at work phase 6 (§ 4.5): rank 1 to the top `1 + members /
    // lieutenant_per` of the ranking after the leader (none off).
    let lieutenants: Vec<EntityId> = if crate::systems::careers::lieutenants_on(world) {
        let n = crate::systems::careers::lieutenant_count(world, members.len());
        ranking.iter().copied().filter(|&m| Some(m) != leader).take(n).collect()
    } else {
        Vec::new()
    };
    for &m in &members {
        if let Some(gm) = world.comp_mut::<GangMember>(m) {
            gm.rank = if Some(m) == leader {
                2
            } else if lieutenants.contains(&m) {
                1
            } else {
                0
            };
        }
    }
    // A leader who lost the post to a Sentence is the boss until they are out.
    // The standing boss is kept while they are still inside.
    let jailed_boss = old.filter(|&o| leader != Some(o) && world.has::<Sentence>(o));
    let boss_inside = world.comp::<Gang>(gang).and_then(|g| g.boss).is_some_and(|b| world.has::<Sentence>(b));
    let decapitated = !war_lost && fallen.is_some() && old == fallen && leader.is_some() && leader != old;
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.leader = leader;
        if kept.is_none() && g.war.is_none() {
            g.crowned = None;
        }
        // A claimant who lost the war from a cell is nobody's boss.
        if jailed_boss.is_some() && !boss_inside && !war_lost {
            g.boss = jailed_boss;
        }
        if decapitated {
            g.split_check = old;
        }
    }
    if old.is_some() && leader != old {
        push_shock(world, gang, Shock::LeaderChanged);
    }
    // Pressure pass round 9, Y2 (`OPEN-250`): nobody takes the chair from
    // the cells (0 is right).
    if leader != old && leader.is_some_and(|n| world.has::<Sentence>(n)) {
        crate::systems::ownership::jobs2_row(world).cell_takeovers += 1;
    }
    // Round 8 X4 (`OPEN-241`): a change no other line tells (a decapitation
    // is `succession::decide`'s, a civil war's its own) is told here.
    if let (Some(o), Some(n)) = (old, leader) {
        if o != n {
            crate::systems::ownership::jobs2_row(world).leader_changes += 1;
            if !decapitated && !war_lost && !at_war && crowned.is_none() {
                let (sn, so) = (score(n), score(o));
                told_leader(world, gang, o, n, back == Some(n), sn, so);
            }
        }
    }
    // Round 3 S2 (OPEN-222): a Lead walk for a post lost turns back.
    if crate::systems::leaders::on(world) {
        crate::systems::leaders::posts_changed(world, gang);
    }
}

/// A member is out of the Jail (served, player-released or broken out):
/// they are nobody's boss any more and may lead again.
pub fn on_member_released(world: &mut World, id: EntityId) {
    let Some(gang) = world.gang_of(id) else { return };
    let mut back = None;
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        if g.boss == Some(id) {
            g.boss = None;
            back = Some(id);
        }
    }
    // Round 8 X4: the boss walks back into the chair.
    recompute_leader_back(world, gang, None, false, back);
    // The pressure pass phase 5b: the gang meets him (a welcome, or a
    // beating for the one who talked).
    crate::systems::crew::on_released(world, id);
}

/// Round 8 X4 (`OPEN-241`): the gang's members who hear a change at its
/// head: its living adults with a Brain (the ones inside too). Never a minor
/// (CLAUDE.md, the under-18 rule).
pub fn members_told(world: &World, gang: EntityId) -> Vec<EntityId> {
    world.comp::<Gang>(gang).map_or_else(Vec::new, |g| {
        g.members
            .iter()
            .copied()
            .filter(|&m| {
                crate::systems::law::living(world, m)
                    && world.has::<Brain>(m)
                    && !crate::systems::ages::is_protected(world, m)
            })
            .collect()
    })
}

/// Pressure pass round 9, Y2 (`OPEN-250`): `n` runs `gang` while `o` sits
/// inside, and the boss whose chair it is (`Gang.boss`, jailed before `o`,
/// who walks back into it on release) is named too: V17 `purist_32297 d24
/// 11:43` read "Smoke Wozniak is out of the cells and runs it again" after
/// "Annika Achebe runs it while Alaric Fairweather sits inside", Smoke in
/// the cells since day 18 and never named.
pub fn inside_line(world: &World, gang: EntityId, n: EntityId, o: EntityId) -> String {
    let gname = world.comp::<Gang>(gang).map_or_else(|| "the gang".to_string(), |g| g.name.clone());
    let (nn, on) = (world.name_of(n), world.name_of(o));
    let boss = world
        .comp::<Gang>(gang)
        .and_then(|g| g.boss)
        .filter(|&b| b != o && b != n && world.has::<Sentence>(b) && world.gang_of(b) == Some(gang));
    match boss {
        Some(b) => {
            let him = crate::systems::consequence::pro(world, o).1;
            format!("{nn} runs {gname} while {on} sits inside, as {} has since before {him}", world.name_of(b))
        }
        None => format!("{nn} runs {gname} while {on} sits inside"),
    }
}

/// Round 8 X4 (`OPEN-241`): the new leader `n` of `gang` after `o`, told to
/// its members (a `Succession` line) and to the street (a `TookOver` deed in
/// the Hideout's district). V16: `raider_29990` read "Rhys Garland leads 23"
/// one night and "Jasper Marlow leads 23" the next, with Rhys still about.
pub fn told_leader(world: &mut World, gang: EntityId, o: EntityId, n: EntityId, back: bool, sn: f32, so: f32) {
    let gname = world.comp::<Gang>(gang).map_or_else(|| "the gang".to_string(), |g| g.name.clone());
    let (nn, on) = (world.name_of(n), world.name_of(o));
    let him = crate::systems::consequence::pro(world, n).1;
    let text = if back {
        format!("{nn} is out of the cells and runs {gname} again ({on} stood in for {him})")
    } else if !crate::systems::law::living(world, o) {
        format!("{nn} took over {gname} after {on} died")
    } else if world.has::<Sentence>(o) {
        inside_line(world, gang, n, o)
    } else if world.gang_of(o) != Some(gang) {
        format!("{nn} took over {gname} after {on} left it")
    } else {
        let why = if leader_veteran(world, n) && !leader_veteran(world, o) {
            format!("the old hands follow {him}, not a newcomer")
        } else {
            format!("the members follow {him} now ({sn:.2} to {so:.2})")
        };
        format!("{nn} pushed {on} aside and runs {gname}: {why}")
    };
    let mut actors = vec![n, gang, o];
    actors.extend(members_told(world, gang));
    let mut seen = std::collections::BTreeSet::new();
    actors.retain(|&a| seen.insert(a));
    world.push_event(EventKind::Succession, &actors, text);
    crate::systems::ownership::jobs2_row(world).leader_told += 1;
    tell_street(world, gang, n);
}

/// Round 8 X4: the street hears who runs `gang` now (a `TookOver` deed in
/// its Hideout's district).
pub fn tell_street(world: &mut World, gang: EntityId, n: EntityId) {
    let door = world.hideout_of(gang).and_then(|h| world.comp::<crate::components::Building>(h)).map(|b| b.door);
    if let Some(d) = door.map(|t| world.district_of(t)) {
        crate::systems::gossip::post_deed(world, d, crate::word::Deed::TookOver, Some(n), Some(gang));
    }
}

/// Members serving a sentence (a Vagrancy night excluded).
pub fn jailed_headcount(world: &World, gang: EntityId) -> usize {
    // M12 D15: a night in the cells for Vagrancy is no reason to storm the Jail.
    let held = |m: EntityId| world.comp::<Sentence>(m).is_some_and(|s| s.crime != crate::components::Crime::Vagrancy);
    world.comp::<Gang>(gang).map_or(0, |g| g.members.iter().filter(|&&m| held(m)).count())
}

/// Members not in the Jail.
pub fn fit_headcount(world: &World, gang: EntityId) -> usize {
    world.comp::<Gang>(gang).map_or(0, |g| g.members.iter().filter(|&&m| !world.has::<Sentence>(m)).count())
}

// ---------------------------------------------------------------------------
// Heat and shocks
// ---------------------------------------------------------------------------

pub fn push_shock(world: &mut World, gang: EntityId, shock: Shock) {
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.shocks.push(shock);
    }
}

/// A member's killer turned out to be a rival (an off-screen killing bound
/// later): the pending `MemberKilled { by_rival: false }` becomes
/// `by_rival: true`; once the brain has consumed it, only the difference
/// arrives (`Shock::RivalNamed`). One death is never two shocks.
pub fn upgrade_kill_shock(world: &mut World, gang: EntityId) {
    let Some(g) = world.comp_mut::<Gang>(gang) else { return };
    if let Some(s) = g.shocks.iter_mut().find(|s| **s == Shock::MemberKilled { by_rival: false }) {
        *s = Shock::MemberKilled { by_rival: true };
    } else {
        g.shocks.push(Shock::RivalNamed);
    }
}

fn log_heat(world: &mut World, gang: EntityId, member: EntityId) {
    let tick = world.tick;
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        if g.heat_log.len() >= HEAT_LOG_CAP {
            g.heat_log.pop_front();
        }
        g.heat_log.push_back((tick, member));
    }
}

/// `law::sentence` jailed a member: heat, a shock, and maybe a new leader.
pub fn on_member_arrested(world: &mut World, id: EntityId) {
    let Some(gang) = world.gang_of(id) else { return };
    // Round 6 V5: a war side's loss.
    crate::systems::turf::note_loss(world, gang, id);
    log_heat(world, gang, id);
    push_shock(world, gang, Shock::MemberArrested);
    recompute_leader_after(world, gang, Some(id));
    // The pressure pass phase 5b: the sentence's start (the talk, the welcome).
    crate::systems::crew::on_arrested(world, id);
}

/// `World::kill_by`, before the components go: the member leaves the roster;
/// the gang logs the heat and the shock (`by_rival` when a rival member did it).
pub fn on_member_killed(world: &mut World, id: EntityId, killer: Option<EntityId>) {
    let Some(gang) = world.gang_of(id) else { return };
    // Round 6 V5: a war side's loss.
    crate::systems::turf::note_loss(world, gang, id);
    crate::systems::ownership::jobs2_row(world).gang_deaths += 1;
    let by_rival = killer.and_then(|k| world.gang_of(k)).is_some_and(|k| k != gang);
    let tick = world.tick;
    if let Some(g) = world.comp_mut::<Gang>(gang) {
        g.members.retain(|&m| m != id);
        if g.members.is_empty() {
            g.empty_since = Some(tick);
            g.emptied = true;
        }
        if g.boss == Some(id) {
            g.boss = None;
        }
    }
    log_heat(world, gang, id);
    push_shock(world, gang, Shock::MemberKilled { by_rival });
    recompute_leader_after(world, gang, Some(id));
}
