use super::*;

// ---------------------------------------------------------------------------
// Secrets and blackmail
// ---------------------------------------------------------------------------

/// Is `d` a deed a secret can be (spec: `deed_sev ≥ 0.4` or `honour_w < 0`)?
fn shameful(world: &World, d: Deed) -> bool {
    world.config.gossip.deed_sev.get(d) >= 0.4 || world.config.reputation.honour_w.get(d) < 0.0
}

/// The deeds `holder` knows first-hand or at one hop (`memory::deed_of`
/// over its first-hand memories and its heard store) that a secret can be,
/// with a living person as the actor other than the holder.
pub fn secrets_of(world: &World, holder: EntityId) -> SmallVec<[DeedRef; 8]> {
    let mut out: SmallVec<[DeedRef; 8]> = SmallVec::new();
    let Some(m) = world.comp::<Memory>(holder) else { return out };
    for e in m.entries.iter().chain(m.heard.iter()) {
        if e.hops > 1 {
            continue;
        }
        let Some(r) = crate::systems::memory::deed_of(holder, e) else { continue };
        let Some(a) = r.actor else { continue };
        if a == holder || !world.has::<Identity>(a) || !shameful(world, r.deed) {
            continue;
        }
        if !out.iter().any(|x| x.deed == r.deed && x.actor == r.actor && x.object == r.object) {
            out.push(r);
        }
    }
    out
}

/// How many agents hold each of `keys` (any hops): one pass over the
/// memories (the spec's daily tally of the keys referenced, run only on a
/// day a key is asked for).
pub fn holders(world: &World, keys: &[DeedRef]) -> Vec<u32> {
    let mut n = vec![0u32; keys.len()];
    if keys.is_empty() {
        return n;
    }
    // scan-ok: daily, only when a secret is weighed.
    for id in world.with::<Memory>() {
        let Some(m) = world.comp::<Memory>(id) else { continue };
        let mut seen = vec![false; keys.len()];
        for e in m.entries.iter().chain(m.heard.iter()) {
            let Some(r) = crate::systems::memory::deed_of(id, e) else { continue };
            for (i, k) in keys.iter().enumerate() {
                if !seen[i] && k.deed == r.deed && k.actor == r.actor && k.object == r.object {
                    seen[i] = true;
                    n[i] += 1;
                }
            }
        }
    }
    n
}

/// A secret `holder` holds on `over` (a deed `over` did, known to few and
/// not yet public), the most severe; `None` if none.
pub fn secret_on(world: &World, holder: EntityId, over: EntityId) -> Option<Leverage> {
    let cfg = &world.config.leverage;
    let mine: SmallVec<[DeedRef; 8]> =
        secrets_of(world, holder).into_iter().filter(|r| r.actor == Some(over)).collect();
    let counts = holders(world, &mine);
    let day = u32::try_from(world.day()).unwrap_or(u32::MAX);
    mine.iter()
        .zip(counts)
        .filter(|&(r, n)| n <= cfg.secret_known_max && reach(world, r.deed, over, r.object) < cfg.secret_public_reach)
        .max_by(|a, b| {
            let (sa, sb) = (world.config.gossip.deed_sev.get(a.0.deed), world.config.gossip.deed_sev.get(b.0.deed));
            sa.total_cmp(&sb).then(b.0.deed.cmp(&a.0.deed))
        })
        .map(|(r, _)| Leverage::Secret { deed: r.deed, actor: over, object: r.object, day })
}

fn under_blackmail(world: &World, id: EntityId) -> bool {
    world
        .coercions
        .values()
        .any(|c| c.is_active() && c.why == CoercionWhy::Blackmail && (c.over == id || c.agent == id))
}

/// Daily: the adults whose day it is (`index % blackmail_days`), under
/// `blackmail_lawfulness`, free, holding no blackmail: each looks for a
/// secret on someone richer (`standing` above its own by `blackmail_gap`)
/// known to at most `secret_known_max` and not public, and makes the
/// richest of them pay `blackmail_weekly` a week.
pub(super) fn blackmail_pass(world: &mut World) {
    let cfg = world.config.leverage.clone();
    let day = world.day();
    let period = u64::from(cfg.blackmail_days.max(1));
    let mut cands: Vec<(EntityId, EntityId, DeedRef)> = Vec::new();
    // scan-ok: daily, one adult in `blackmail_days`.
    for a in world.citizens() {
        if !(u64::from(a.index) + day).is_multiple_of(period) {
            continue;
        }
        if world.comp::<Personality>(a).is_none_or(|p| p.lawfulness >= cfg.blackmail_lawfulness) || !free(world, a) {
            continue;
        }
        if under_blackmail(world, a) {
            continue;
        }
        let mine = reputation::rep(world, a).standing;
        let best = secrets_of(world, a)
            .into_iter()
            .filter_map(|r| r.actor.map(|t| (t, r)))
            .filter(|&(t, _)| law::living(world, t) && !world.has::<Sentence>(t) && !under_blackmail(world, t))
            .filter(|&(t, _)| !tried_recently(world, a, t, tried::BLACKMAIL))
            .map(|(t, r)| (reputation::rep(world, t).standing, t, r))
            .filter(|&(s, _, _)| s > mine + cfg.blackmail_gap)
            .max_by(|x, y| x.0.total_cmp(&y.0).then(y.1.cmp(&x.1)));
        if let Some((_, t, r)) = best {
            cands.push((a, t, r));
        }
    }
    if cands.is_empty() {
        return;
    }
    let keys: Vec<DeedRef> = cands.iter().map(|c| c.2).collect();
    let counts = holders(world, &keys);
    let day32 = u32::try_from(day).unwrap_or(u32::MAX);
    for ((a, t, r), n) in cands.into_iter().zip(counts) {
        if !room(world) {
            return;
        }
        if n > cfg.secret_known_max || reach(world, r.deed, t, r.object) >= cfg.secret_public_reach {
            continue;
        }
        if under_blackmail(world, t) || under_blackmail(world, a) {
            continue;
        }
        let lev = Leverage::Secret { deed: r.deed, actor: t, object: r.object, day: day32 };
        try_blackmail(world, a, t, lev);
    }
}

/// One blackmail move by `holder` on `over` with `lev` (a Secret). True
/// when made: the hush money is due weekly from now.
pub fn try_blackmail(world: &mut World, holder: EntityId, over: EntityId, lev: Leverage) -> bool {
    if !on(world) {
        return false;
    }
    mark(world, holder, over, tried::BLACKMAIL);
    let amount = world.config.leverage.blackmail_weekly;
    let demand = Demand::Coins { amount, weekly: true };
    let out = roll(world, holder, over, None, &lev, &demand);
    if !out.success {
        return false;
    }
    let id = record(world, Some(holder), holder, over, lev, demand, CoercionWhy::Blackmail, None, None);
    world.stats.current.leverage.blackmail_made += 1;
    let d = crate::systems::gossip::talk_district(world, holder);
    crate::systems::gossip::post_deed(world, d, Deed::Blackmailed, Some(holder), Some(over));
    collect_one(world, id);
    true
}

/// Daily: a blackmail lapses (no backlash) once any pool entry of its deed
/// reaches `secret_public_reach`: the secret is out.
pub(super) fn lapse(world: &mut World) {
    let bound = world.config.leverage.secret_public_reach;
    let out: Vec<CoercionId> = world
        .coercions
        .values()
        .filter(|c| c.is_active())
        .filter_map(|c| match c.leverage {
            Leverage::Secret { deed, actor, object, .. } => {
                (reach(world, deed, actor, object) >= bound).then_some(c.id)
            }
            _ => None,
        })
        .collect();
    for id in out {
        close(world, id, CoercionStatus::Lapsed, "public");
    }
}

// ---------------------------------------------------------------------------
// Threats: Silence and the coerced job
// ---------------------------------------------------------------------------

/// Silence (spec § 4 Threat), daily before the Hit market's Silence pass:
/// a free suspect under `silence_lawfulness` with an open report whose
/// witness it knows leans on the witness to withdraw it (one try per pair
/// per `retry_days`). A success resolves every open report the witness
/// filed on the suspect, so the Hit market finds nothing to buy; a failure
/// leaves it to the Beat. Gang members included (a threat is personal).
pub fn silence_pass(world: &mut World) {
    if !on(world) {
        return;
    }
    let max = world.config.leverage.silence_lawfulness;
    for (_, s, w, _) in crate::systems::hitmarket::silenceable(world) {
        if world.comp::<Personality>(s).is_none_or(|p| p.lawfulness >= max) {
            continue;
        }
        if tried_recently(world, s, w, tried::SILENCE) || !law::living(world, w) {
            continue;
        }
        try_silence(world, s, w);
    }
}

/// One Silence move by suspect `s` on witness `w`. True when withdrawn.
pub fn try_silence(world: &mut World, s: EntityId, w: EntityId) -> bool {
    if !on(world) {
        return false;
    }
    let Some(tick) = world
        .crime_reports()
        .iter()
        .filter(|r| !r.resolved && r.suspect == s && r.witness == Some(w))
        .map(|r| r.tick)
        .max()
    else {
        return false;
    };
    mark(world, s, w, tried::SILENCE);
    let demand = Demand::Silence { suspect: s, tick };
    if !roll(world, s, w, None, &Leverage::Threat, &demand).success {
        return false;
    }
    let idx: Vec<usize> = world
        .crime_reports()
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.resolved && r.suspect == s && r.witness == Some(w))
        .map(|(i, _)| i)
        .collect();
    for i in idx {
        if let Some(r) = world.reports_mut().get_mut(i) {
            r.resolved = true;
        }
    }
    world.stats.current.leverage.silenced_threat += 1;
    let text = format!("{} leaned on {}: the report was withdrawn", world.name_of(s), world.name_of(w));
    world.push_event(EventKind::Coerced, &[s, w], text);
    let d = crate::systems::gossip::talk_district(world, w);
    crate::systems::gossip::post_deed(world, d, Deed::Extorted, Some(s), Some(w));
    true
}
