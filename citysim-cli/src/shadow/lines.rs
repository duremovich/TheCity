use super::*;

/// The activity class of the agent this tick.
pub(super) fn activity(world: &World, id: EntityId) -> &'static str {
    if world.has::<Sentence>(id) {
        // Pressure pass round 3, S3: lights-out in the cells is sleep.
        if citysim::systems::law::asleep_in_cell(world, id) {
            return "sleep";
        }
        return "jail";
    }
    let Some(b) = world.comp::<Brain>(id) else { return "idle" };
    // V2: a Statistical stand-in (an unpinned run) has no step to classify.
    if b.lod == Lod::Statistical {
        return "stat";
    }
    if matches!(b.exec, ExecState::JackedIn { .. }) {
        return "crime";
    }
    if matches!(b.exec, ExecState::Goto { .. } | ExecState::GotoTimed { .. } | ExecState::Fly { .. }) {
        // Lives at work phase 3 (§ 2.2): a round leg on shift is the work.
        // Lives at work phase 5 (§ 2.4): a reporter's beat leg, likewise.
        // The pre-view pass phase 2 (OPEN-137): a stations shift's walk
        // across the room between its spots, likewise.
        if b.plan_goal() == Some(GoalKind::Work)
            && (citysim::systems::trades::on_round(world, id)
                || citysim::systems::news::on_beat(world, id)
                || b.current_step().is_some_and(|s| s.action == ActionKind::GoTo(citysim::LocationKey::Station))
                // Shadow V7 (tool): any leg of a shape plan (a haul trip,
                // the walk out to the break) is on shift too.
                || citysim::systems::shift::scripted(world, id))
        {
            return "work";
        }
        return "travel";
    }
    let Some(step) = b.plan.as_ref().and_then(|_| b.current_step()) else { return "idle" };
    use ActionKind as A;
    match step.action {
        A::Sleep | A::CheckIn => "sleep",
        A::Rest if b.plan_goal() == Some(GoalKind::Sleep) => "sleep",
        // Lives at work phase 4 (§ 2.3): a cab driver standing by at the Garage.
        A::Rest if b.plan_goal() == Some(GoalKind::Work) && citysim::systems::streetwork::standing_by(world, id) => {
            "work"
        }
        A::Rest => "idle",
        A::FarmWork
        | A::HaulToMarket
        | A::ClerkWork
        | A::BartendWork
        | A::CollectWage
        | A::SellFood
        | A::PatrolLeg
        | A::Arrest
        | A::Escort
        | A::GuardJail
        | A::TendGraves
        | A::BuryCorpse
        | A::CarryCorpse
        | A::ReportCrime
        | A::Deal
        | A::PickUp
        | A::SellData
        | A::Register
        | A::FabWork
        | A::Sweep
        | A::Collect
        // Shadow V7 (tool): the pre-view pass's scripted steps (a stations
        // shift's break, a haul's unload, the leader's court) were `other`.
        | A::Break
        | A::Unload
        | A::Court => "work",
        A::Enjoy | A::Gamble | A::EatOut | A::HangOut => "leisure",
        A::EatFromInventory | A::EatAtHome | A::BuyFood | A::Forage | A::StoreFood => "eat",
        A::Chat | A::Drink | A::Flirt | A::Propose | A::AskAround | A::Preach => "social",
        A::StealFood(_)
        | A::Attack
        | A::Extort
        | A::SplitLoot
        | A::Muster
        | A::Brawl
        | A::StealVehicle
        | A::Strip
        | A::Rip
        | A::Abduct
        | A::Fence
        | A::HideFromLaw
        | A::JackIn
        | A::BuyStims
        | A::UseStim
        | A::StakeOut => "crime",
        A::Wander | A::Beg | A::CollectDole => "idle",
        A::GoTo(_) => "travel",
        A::Meeting => "meeting",
        _ => "other",
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

pub(super) fn header(world: &World, t: &Track) -> String {
    let id = t.id;
    let mut h = String::new();
    let _ = writeln!(h, "# {} ({}) - {}#{}", t.name, t.arch, t.arch, id.index);
    let _ = writeln!(h);
    let Some(ident) = world.comp::<Identity>(id) else { return h };
    let sex = if ident.sex == Sex::Male { "male" } else { "female" };
    let _ = writeln!(h, "- **Who**: {} y {}, class {}", ident.age_years(), sex, class_of(world, id).label());
    let _ = writeln!(h, "- **Where**: {}", place(world, id));
    if let Some(p) = world.comp::<Personality>(id) {
        let _ = writeln!(
            h,
            "- **Personality**: lawfulness {:.2}, greed {:.2}, pride {:.2}, sociability {:.2}, courage {:.2}, loyalty {:.2}",
            p.lawfulness, p.greed, p.pride, p.sociability, p.courage, p.loyalty
        );
    }
    if let Some(s) = world.comp::<Skills>(id) {
        let _ = writeln!(
            h,
            "- **Skills**: stealth {:.2}, melee {:.2}, shooting {:.2}, evasion {:.2} (power {:.2}), farming {:.2}, hacking {:.2} | persuasion {:.2}, intimidation {:.2}, knowledge {:.2}, deception {:.2}",
            s.stealth,
            s.melee,
            s.shooting,
            s.evasion,
            citysim::systems::combat::power(world, id),
            s.farming,
            s.hacking,
            s.persuasion,
            s.intimidation,
            s.knowledge,
            s.deception
        );
    }
    // The combat model C1: the wounds carried at the window's start.
    if let Some(w) = world.comp::<citysim::Wounds>(id) {
        let list: Vec<String> = w
            .list
            .iter()
            .map(|x| {
                format!("{} {}{}", x.region.label(), x.severity.label(), if x.bleeding { ", bleeding" } else { "" })
            })
            .collect();
        let marks: Vec<String> = w.marks.iter().map(|(r, m)| format!("{} {}", r.label(), m.label())).collect();
        let marks = if marks.is_empty() { String::new() } else { format!("; marks: {}", marks.join(", ")) };
        let _ = writeln!(h, "- **Wounds**: {} (blood {:.2}){marks}", list.join("; "), w.blood);
    }
    let home = world.comp::<Household>(id).and_then(|h| h.home);
    let _ = writeln!(
        h,
        "- **Home**: {}",
        match home {
            // C6a: his flat and its storey, the Home's floors.
            Some(b) => {
                let flat = citysim::systems::floors::flat_of(world, id)
                    .zip(world.comp::<Building>(b))
                    .map_or(String::new(), |(f, bd)| {
                        format!(", flat {} on floor {}", f + 1, citysim::systems::floors::flat_floor(bd, b, f))
                    });
                format!("{}{flat}{}", place_of_building(world, b), floors_of(world, b))
            }
            None => "none (homeless)".into(),
        }
    );
    if let Some(j) = world.comp::<Job>(id) {
        let _ = writeln!(
            h,
            "- **Job**: {} at {}{} (wage {}/day, shifts {:?})",
            j.role.label(),
            j.employer.map_or("?".into(), |e| nm(world, e)),
            j.employer.map_or(String::new(), |e| floors_of(world, e)),
            j.wage_per_day,
            j.shifts
        );
    } else {
        let _ = writeln!(h, "- **Job**: none");
    }
    // The pressure pass F1: the doors and the structural commute between them.
    let door = |b: EntityId| world.comp::<Building>(b).map(|bd| bd.door);
    let home_door = home.and_then(door);
    let work_door = world.comp::<Job>(id).and_then(|j| j.employer).and_then(door);
    if let (Some(hd), Some(wd)) = (home_door, work_door) {
        let _ = writeln!(
            h,
            "- **Commute**: Home door ({}, {}) to work door ({}, {}): {} tiles, travel {} min",
            hd.x,
            hd.y,
            wd.x,
            wd.y,
            hd.manhattan(wd),
            citysim::systems::distance::travel(world, hd, wd)
        );
    }
    let owned: Vec<String> = world
        .buildings_by_kind()
        .values()
        .flatten()
        .copied()
        .filter(|&b| world.comp::<Building>(b).is_some_and(|bl| bl.owner == Some(id)))
        // The city milestone C6b: a tenancy's floors and floor rent.
        .map(|b| citysim::systems::floors::lease_text(world, b).unwrap_or_else(|| bname(world, b)))
        .collect();
    if !owned.is_empty() {
        let _ = writeln!(h, "- **Owns**: {}", owned.join(", "));
    }
    if let Some(g) = world.gang_of(id).and_then(|g| world.comp::<Gang>(g).map(|c| (g, c))) {
        let rank = world.comp::<GangMember>(id).map_or(0, |m| m.rank);
        let role = if g.1.leader == Some(id) { "LEADER" } else { "member" };
        let _ = writeln!(
            h,
            "- **Gang**: {} ({role}, rank {rank}), creed {:?}, order {:?}, treasury {}, {} members",
            g.1.name,
            g.1.creed,
            g.1.order,
            money(g.1.treasury),
            g.1.members.len()
        );
    }
    if let Some(c) = world.corp_of_agent(id).and_then(|c| world.comp::<Corp>(c).map(|k| (c, k))) {
        let exec = if c.1.exec == Some(id) { "EXEC" } else { "staff" };
        let _ =
            writeln!(h, "- **Corp**: {} ({exec}), treasury {}, order {:?}", c.1.name, money(c.1.treasury), c.1.order);
    }
    let wallet = world.comp::<Wallet>(id).map_or(0, |w| w.coins);
    let inv = world.comp::<Inventory>(id).map(|i| format!("food {}, stims {}, parts {}", i.food, i.stims, i.parts));
    let _ = writeln!(h, "- **Wallet**: {} | inventory: {}", money(wallet), inv.unwrap_or_default());
    // Pressure pass round 2, R1: the fare record (caught at the gate, the El's ban).
    if let Some(r) = world.fares.book.get(&id) {
        let ban = r.banned_until.filter(|&t| t > world.tick).map_or_else(
            || "not banned".to_string(),
            |t| format!("banned from the El until d{}", t / citysim::time::TICKS_PER_DAY),
        );
        let _ = writeln!(h, "- **Fares**: caught at the gate {} time(s) lately, {ban}", r.caught.len());
    }
    if let Some(hl) = world.comp::<citysim::Health>(id) {
        let _ = writeln!(h, "- **Health**: {}", health_line(hl));
    }
    if let Some(k) = world.comp::<Kit>(id) {
        let assets: Vec<String> =
            world.assets_by_owner.get(&id).map(|v| v.iter().map(|&a| world.name_of(a)).collect()).unwrap_or_default();
        let _ = writeln!(
            h,
            "- **Kit**: fighting {:.2}, reflex {:.2}, strength {:.2}, armour {:.2}, chrome {} (value {}), vehicle {}, deck tier {} | assets: {}",
            k.fighting,
            k.reflex,
            k.strength,
            k.armour,
            k.chrome,
            k.chrome_value,
            k.vehicle.map_or("none".into(), |v| world.name_of(v)),
            k.deck_tier,
            if assets.is_empty() { "none".into() } else { assets.join(", ") }
        );
    }
    if let Some(b) = world.comp::<Body>(id) {
        let _ = writeln!(h, "- **Body**: sanity {:.2}, addiction {:.2}", b.sanity, b.addiction);
    }
    let r = rep(world, id);
    let _ = writeln!(
        h,
        "- **Reputation**: dread {:.2}, standing {:.2}, honour {:.2}, heat {:.2}, known by {}",
        r.dread, r.standing, r.honour, r.heat, r.known_by
    );
    // Key relationships: spouse, kin, enemies, rivals, then the warmest.
    let mut es: Vec<(EntityId, f32, f32, RelKind, i32)> = world
        .neighbours(id)
        .filter_map(|o| world.edge(id, o).map(|e| (o, e.affinity, e.trust, e.kind, e.debt)))
        .collect();
    es.sort_by(|a, b| {
        let strong = |k: RelKind| {
            matches!(k, RelKind::Spouse | RelKind::Family | RelKind::Parent | RelKind::Enemy | RelKind::Rival)
        };
        strong(b.3).cmp(&strong(a.3)).then(b.1.abs().total_cmp(&a.1.abs())).then(a.0.cmp(&b.0))
    });
    let _ = writeln!(h, "- **Relationships** ({} edges), top:", es.len());
    for (o, aff, trust, kind, debt) in es.into_iter().take(10) {
        let _ = writeln!(
            h,
            "  - {} [{:?}] affinity {:+.2}, trust {:.2}{}",
            nm(world, o),
            kind,
            aff,
            trust,
            if debt != 0 { format!(", debt {debt}") } else { String::new() }
        );
    }
    let _ = writeln!(h);
    h
}

// ---------------------------------------------------------------------------
// Per-tick observation
// ---------------------------------------------------------------------------

pub(super) fn flow_kind(action: Option<ActionKind>, midnight: bool) -> &'static str {
    use ActionKind as A;
    if midnight {
        return "daily pass (wage/rent/upkeep/tax)";
    }
    match action {
        // L1: a shift's wage is paid as the shift ends.
        Some(
            A::CollectWage
            | A::ClerkWork
            | A::FarmWork
            | A::BartendWork
            | A::TendGraves
            | A::GuardJail
            | A::FabWork
            | A::Sweep,
        ) => "wage",
        Some(A::Enjoy) => "venue entry",
        Some(A::EatOut) => "venue meal",
        Some(A::Gamble) => "gambling",
        Some(A::Collect) => "collect (tribute/fronts)",
        Some(A::Scavenge) => "scavenging",
        Some(A::Meeting) => "office",
        Some(A::BuyFood) => "food purchase",
        Some(A::SellFood | A::HaulToMarket) => "food sale",
        Some(A::Fence) => "fenced goods",
        Some(A::Beg) => "begging",
        Some(A::CollectDole) => "dole",
        Some(A::Extort | A::SplitLoot | A::Strip) => "loot/extortion/split",
        Some(A::BuyAsset) => "asset purchase",
        Some(A::BuyStims | A::UseStim) => "stims purchase",
        Some(A::Deal) => "stims sale",
        Some(A::Install | A::Therapy | A::Detox | A::Uninstall) => "clinic",
        Some(A::CheckIn) => "hotel bed",
        Some(A::Register) => "founding fee",
        Some(A::Drink) => "bar tab",
        Some(A::SellData) => "data sale",
        Some(A::UpgradeDeck) => "deck upgrade",
        Some(A::Arrest | A::Escort) => "law",
        _ => "other",
    }
}

pub(super) fn goal_line(world: &World, id: EntityId, b: &Brain) -> String {
    let Some(tr) = b.last_think.as_ref() else { return "(no think trace)".into() };
    let chosen = b.current_goal;
    let mut s = String::new();
    if let Some(top) = tr.goals.first() {
        if Some(top.goal) != chosen {
            let _ = write!(s, "(trace winner {:?} deferred by an uninterruptible step) ", top.goal);
        }
    }
    let score = tr.goals.iter().find(|g| Some(g.goal) == chosen);
    match score {
        Some(g) => {
            let _ = write!(s, "score {:.3}", g.score);
        }
        None => {
            let _ = write!(s, "score ?");
        }
    }
    let runners: Vec<String> = tr
        .goals
        .iter()
        .filter(|g| Some(g.goal) != chosen)
        .take(3)
        .map(|g| format!("{:?} {:.3}", g.goal, g.score))
        .collect();
    let _ = write!(s, "; runners-up: {}", if runners.is_empty() { "none".into() } else { runners.join(", ") });
    if let Some(g) = score {
        let cs: Vec<String> =
            g.considerations.iter().map(|c| format!("{} {:.2}->{:.2}", c.name, c.input, c.output)).collect();
        let _ = write!(s, "; because {}", cs.join(", "));
    }
    if chosen != Some(GoalKind::Work) {
        if let Some(why) = work_blocked(world, id) {
            let _ = write!(s, "; Work: {why}");
        }
    }
    s
}

/// The perf pass phase 1 (OPEN-195): why the Work goal cannot run, for the
/// goal line: an exec whose corp has no office (`life::hq_of` skips Homes
/// and Lots: a housing corp's exec never plans Work), a post with no
/// workplace, or the Work goal's refusals (`routine::work_refused`).
fn work_blocked(world: &World, id: EntityId) -> Option<&'static str> {
    use citysim::systems::life;
    let Some(job) = world.comp::<Job>(id) else {
        let corp = life::exec_corp(world, id)?;
        return life::office_of(world, corp, id).is_none().then_some("no Hq");
    };
    if job.employer.is_none() {
        Some("no workplace")
    } else if world.has::<Sentence>(id) {
        Some("sentenced")
    } else if citysim::systems::health::sick(world, id) {
        Some("sick")
    } else if job.days_unpaid >= 7 {
        Some("unpaid a week")
    } else {
        None
    }
}

pub(super) fn plan_line(world: &World, b: &Brain) -> Option<String> {
    let p = b.plan.as_ref()?;
    let steps: Vec<String> = p
        .steps
        .iter()
        .map(|s| match s.target {
            Some(t) if !matches!(s.action, ActionKind::GoTo(_)) => format!("{:?}->{}", s.action, nm(world, t)),
            _ => format!("{:?}", s.action),
        })
        .collect();
    Some(format!(
        "{:?}{} [{}]",
        p.goal,
        p.target.map_or(String::new(), |t| format!(" on {}", nm(world, t))),
        steps.join(" > ")
    ))
}

pub(super) fn mem_text(world: &World, holder: EntityId, e: &MemoryEntry) -> String {
    let who = e.subject.map(|s| nm(world, s));
    match (e.kind, e.deed) {
        (MemoryKind::Rumour, Some(d)) => format!(
            "heard a rumour: {:?} by {} on {} (hops {}, conf {:.2})",
            d,
            who.unwrap_or_else(|| "someone unnamed".into()),
            e.object.map_or("?".into(), |o| nm(world, o)),
            e.hops,
            e.conf
        ),
        (MemoryKind::Sighting, _) => format!(
            "sighting of {}{}",
            who.unwrap_or_else(|| "?".into()),
            e.at.map_or(String::new(), |a| format!(" at {}", nm(world, a)))
        ),
        // Lives at work phase 1: the encounter, either side; "a regular" when
        // the holder's ledger (a server's) or the other's (a served's server)
        // holds the pair at two or more visits.
        // Lives at work phase 4 (§ 2.3): a loader's lifts name no person.
        (MemoryKind::Served, _)
            if e.subject.is_none()
                && world
                    .comp::<citysim::Job>(holder)
                    .and_then(|j| citysim::systems::service::row_for(world, j.role))
                    .is_some_and(|r| r.kind == citysim::components::ServeKind::Crate) =>
        {
            let yard = world.comp::<citysim::Job>(holder).and_then(|j| j.employer);
            format!("loaded the shift's crates for {}", world.owner_label(yard.and_then(|y| world.owner_of(y))))
        }
        // Shadow V6 (V5 bug 20): the kind and the side.
        (MemoryKind::Served, _) => served_text(world, holder, e),
        _ => format!(
            "memory {:?}{} (salience {:.2}, valence {:+.2}{}{})",
            e.kind,
            who.map_or(String::new(), |w| format!(" of {w}")),
            e.salience,
            e.valence,
            e.crime.map_or(String::new(), |c| format!(", {c:?}")),
            if e.second_hand { ", second-hand" } else { "" }
        ),
    }
}
