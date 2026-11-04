use super::*;

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// M16a 5.3: the agent's contracts (bought, placed, held or worked, records
/// still on the books: closed ones stay `closed_keep_days`), its regular
/// stamps at the Fixers and the heat of any Fixer it owns, at the run's end.
fn contracts_section(world: &World, id: EntityId, md: &mut String) {
    use citysim::contract::Broker;
    let _ = writeln!(md, "\n### Contracts (M16a)\n");
    let mine: Vec<&citysim::contract::Contract> = world
        .contracts
        .values()
        .filter(|c| c.buyer == Some(id) || c.agent == Some(id) || c.taker == Some(id) || c.crew.contains(&id))
        .collect();
    if mine.is_empty() {
        let _ = writeln!(md, "- records: none");
    }
    for c in mine {
        let role = if c.taker == Some(id) {
            "taker"
        } else if c.crew.contains(&id) {
            "crew"
        } else {
            "buyer"
        };
        let _ = writeln!(
            md,
            "- #{} {} on {} for {} ({role}, {:?}, {:?}, posted {}{}{})",
            c.id,
            c.kind.label(),
            nm(world, c.target.id()),
            money(c.price),
            c.status,
            c.render,
            clock(c.posted),
            c.broker.map(|b| format!(" at {}", bname(world, b))).unwrap_or_default(),
            // The Hit market: an organic record's motive and deposit.
            if c.motive.is_none() {
                String::new()
            } else if c.deposit > 0 {
                format!(", {}, {} down", c.motive.label(), money(c.deposit))
            } else {
                format!(", {}", c.motive.label())
            }
        );
    }
    let fixers: Vec<EntityId> = world.buildings_of_kind(BuildingKind::Fixer).to_vec();
    let stamps: Vec<String> = fixers
        .iter()
        .filter_map(|&f| world.comp::<Broker>(f).and_then(|k| k.regulars.get(&id)).map(|&t| (f, t)))
        .map(|(f, t)| format!("{} (last {})", bname(world, f), clock(t)))
        .collect();
    let _ = writeln!(md, "- regular at: {}", if stamps.is_empty() { "none".into() } else { stamps.join(", ") });
    for f in fixers {
        if world.comp::<Building>(f).is_some_and(|b| b.owner == Some(id)) {
            if let Some(k) = world.comp::<Broker>(f) {
                let _ = writeln!(
                    md,
                    "- owns {}: heat {:.2}, book {}, regulars {}, closed until {}",
                    bname(world, f),
                    k.heat,
                    k.book.len(),
                    k.regulars.len(),
                    k.closed_until.map_or("-".into(), clock)
                );
            }
        }
    }
}

pub(super) fn write_diary(
    world: &World,
    t: &mut Track,
    dir: &std::path::Path,
    end: Tick,
) -> Result<(u64, u64), String> {
    t.entries.sort_by_key(|e| (e.tick, e.seq));
    let stem = format!("{}_{}", t.arch, t.id.index);
    let mut md = t.header.clone();
    md.push('\n');
    md.push_str(&camera_block(t));
    let mut jl = String::new();
    let mut day_open: Option<u64> = None;
    // Summary lines print last in their day.
    let mut by_day: BTreeMap<u64, Vec<&Entry>> = BTreeMap::new();
    for e in &t.entries {
        by_day.entry(e.tick / TICKS_PER_DAY).or_default().push(e);
    }
    for (day, es) in &by_day {
        let _ = day_open.insert(*day);
        // Round 3 S7 (OPEN-225): the rest day in the heading.
        let _ = writeln!(md, "## Day {day}{}\n", t.rest_days.get(day).copied().unwrap_or(""));
        let (sum, rest): (Vec<&&Entry>, Vec<&&Entry>) = es.iter().partition(|e| e.cat == "summary");
        for e in rest.iter().chain(sum.iter()) {
            let line = match e.cat {
                "summary" => format!("**{}**", e.text),
                _ => format!("- `{}` [{}] {}", clock(e.tick), e.cat, e.text),
            };
            let _ = writeln!(md, "{line}");
            let _ = writeln!(
                jl,
                "{{\"tick\":{},\"day\":{},\"time\":{},\"cat\":{},\"text\":{}}}",
                e.tick,
                e.tick / TICKS_PER_DAY,
                json_str(&hm(e.tick)),
                json_str(e.cat),
                json_str(&e.text)
            );
        }
        let _ = writeln!(md);
    }
    // Whole-run summary.
    let _ = writeln!(md, "## Whole-run summary\n");
    if let Some(d) = t.died {
        let _ = writeln!(md, "**Died at {}.** The diary stops there.\n", clock(d));
    } else if !world.has::<Brain>(t.id) && !world.has::<Child>(t.id) {
        let _ = writeln!(md, "(No longer a living agent at the end of the run.)\n");
    }
    let _ = writeln!(md, "### Time use (hours)\n");
    let _ = write!(md, "| day |");
    for c in CLASSES {
        let _ = write!(md, " {c} |");
    }
    let _ = writeln!(md);
    let _ = write!(md, "|---|");
    for _ in CLASSES {
        let _ = write!(md, "---|");
    }
    let _ = writeln!(md);
    for (d, row) in &t.use_by_day {
        let _ = write!(md, "| d{d} |");
        for c in CLASSES {
            let _ = write!(md, " {:.1} |", f64::from(row.get(c).copied().unwrap_or(0)) / 60.0);
        }
        let _ = writeln!(md);
    }
    let _ = write!(md, "| **total** |");
    for c in CLASSES {
        let _ = write!(md, " {:.1} |", f64::from(t.total_use.get(c).copied().unwrap_or(0)) / 60.0);
    }
    let _ = writeln!(md, "\n");
    let _ = writeln!(md, "### Top places (minutes)\n");
    let mut pl: Vec<(&String, &u32)> = t.total_places.iter().collect();
    pl.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (p, m) in pl.into_iter().take(10) {
        let _ = writeln!(md, "- {p}: {m}");
    }
    let _ = writeln!(md, "\n### Top contacts (interactions)\n");
    let mut cs: Vec<&(String, u32)> = t.contacts.values().collect();
    cs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if cs.is_empty() {
        let _ = writeln!(md, "- none recorded");
    }
    for (n, c) in cs.into_iter().take(10) {
        let _ = writeln!(md, "- {n}: {c}");
    }
    let _ = writeln!(md, "\n### Money\n");
    let _ = writeln!(md, "- net {}{}", if t.total_net > 0 { "+" } else { "" }, money(t.total_net));
    for (k, (sum, n)) in &t.flows {
        let _ = writeln!(md, "- {k}: {}{} over {n} flow(s)", if *sum > 0 { "+" } else { "" }, money(*sum));
    }
    let _ = writeln!(md, "\n### By flow kind (the sim's own labels; out / in / count)\n");
    if t.flow_kinds.is_empty() {
        let _ = writeln!(md, "- none recorded");
    }
    for (k, (o, i, n)) in &t.flow_kinds {
        let _ = writeln!(md, "- {k}: out {}, in {}, x{n}", money(*o), money(*i));
    }
    let out_of = |names: &[&str]| -> i64 { names.iter().filter_map(|k| t.flow_kinds.get(*k)).map(|v| v.0).sum() };
    let in_of = |names: &[&str]| -> i64 { names.iter().filter_map(|k| t.flow_kinds.get(*k)).map(|v| v.1).sum() };
    let _ = writeln!(md, "\n### Leisure and the street (L2)\n");
    let _ = writeln!(
        md,
        "- fun activities (Enjoy, Gamble, EatOut, HangOut): {:.1} h",
        f64::from(t.total_use.get("leisure").copied().unwrap_or(0)) / 60.0
    );
    let _ = writeln!(
        md,
        "- coins spent: leisure {} (venues {}, gambling {} less {} won back), drink {}, food {}, rent {}",
        // Street dice are gambling too: stakes lost out, winnings back.
        money(out_of(&["Leisure", "Gamble", "StreetDice"]) - in_of(&["Leisure", "Gamble", "GambleWin", "StreetDice"])),
        money(out_of(&["Leisure"]) - in_of(&["Leisure"])),
        money(out_of(&["Gamble", "StreetDice"]) - in_of(&["Gamble"])),
        money(in_of(&["GambleWin", "StreetDice"])),
        money(out_of(&["Drink"])),
        money(out_of(&["Food"]) - in_of(&["Food"])),
        money(out_of(&["Rent"]))
    );
    let _ = writeln!(md, "- HangOuts: {}, distinct known contacts met there: {}", t.hangouts, t.hang_known.len());
    // Round 7 W2: a tribute line that would read 0c is not printed (V15 street: 22 of 22).
    if t.tribute_in != 0 {
        let _ = writeln!(md, "- tribute received (Flow::Tribute): {}", money(t.tribute_in));
    }
    let _ = writeln!(md, "- gossip told about me: {} telling(s)", t.gossip_about);
    contracts_section(world, t.id, &mut md);
    if t.held_settles > 0 || t.stat_minutes > 0 || !t.stat_notes.is_empty() {
        let _ = writeln!(md, "\n### Statistical stretches (what the stand-in did)\n");
        let _ = writeln!(
            md,
            "- {:.1} h Statistical; {} held-cell settlement(s) covering {:.1} h of jailed decay",
            f64::from(t.stat_minutes) / 60.0,
            t.held_settles,
            t.held_minutes as f64 / 60.0
        );
        for (tk, text) in t.stat_notes.iter().take(40) {
            let _ = writeln!(md, "- `{}` {text}", clock(*tk));
        }
        if t.stat_notes.len() > 40 {
            let _ = writeln!(md, "- ... and {} more", t.stat_notes.len() - 40);
        }
    }
    let _ = writeln!(md, "\n### Notable events\n");
    let kinds: Vec<String> = t.event_kinds.iter().map(|(k, n)| format!("{k} x{n}")).collect();
    let _ = writeln!(md, "- counts: {}", if kinds.is_empty() { "none".into() } else { kinds.join(", ") });
    for (tk, text) in t.notable.iter().take(25) {
        let _ = writeln!(md, "- `{}` {}", clock(*tk), text);
    }
    if t.notable.len() > 25 {
        let _ = writeln!(md, "- ... and {} more", t.notable.len() - 25);
    }
    let _ = writeln!(md, "\n(run ended {})", clock(end));
    let mdp = dir.join(format!("{stem}.md"));
    let jp = dir.join(format!("{stem}.jsonl"));
    std::fs::write(&mdp, &md).map_err(|e| format!("{}: {e}", mdp.display()))?;
    std::fs::write(&jp, &jl).map_err(|e| format!("{}: {e}", jp.display()))?;
    Ok((md.len() as u64, jl.len() as u64))
}
