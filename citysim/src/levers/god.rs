use super::*;

impl World {
    /// The god commands: `Ok((actors, event text))` or why nothing happened.
    pub(super) fn cmd_god(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{law, law_brain};
        let gang_name = |w: &World, g: EntityId| w.comp::<Gang>(g).map(|g| g.name.clone()).ok_or("no such gang");
        match *cmd {
            PlayerCommand::KillAgent(who) => {
                if !self.is_alive(who) || !self.has::<Brain>(who) {
                    return Err("KillAgent: no such agent".into());
                }
                let name = self.name_of(who);
                self.kill_by(who, DeathCause::Violence, None);
                Ok((vec![who], format!("struck {name} dead")))
            }
            PlayerCommand::JailAgent { who, days } => {
                let name = self.name_of(who);
                self.god_jail(who, days)?;
                Ok((vec![who], format!("jailed {name} for {days} days")))
            }
            PlayerCommand::FreeAgent(who) => {
                law::player_release(self, who).map_err(|e| format!("FreeAgent: {e}"))?;
                Ok((vec![who], format!("freed {}", self.name_of(who))))
            }
            PlayerCommand::FundGang { gang, amount } => {
                let name = gang_name(self, gang)?;
                if let Some(g) = self.comp_mut::<Gang>(gang) {
                    g.treasury += amount;
                }
                Ok((vec![gang], format!("gave {name} {amount} coins")))
            }
            PlayerCommand::SeizeGangTreasury(gang) => {
                let name = gang_name(self, gang)?;
                let coins = self.comp_mut::<Gang>(gang).map_or(0, |g| std::mem::take(&mut g.treasury));
                if let Some(t) = self.treasury_mut() {
                    t.coins += coins;
                }
                Ok((vec![gang], format!("seized {coins} coins from {name}")))
            }
            PlayerCommand::KillGang(gang) => {
                let name = gang_name(self, gang)?;
                let members = self.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
                for &m in &members {
                    self.kill_by(m, DeathCause::Violence, None);
                }
                Ok((vec![gang], format!("killed all {} of {name}", members.len())))
            }
            PlayerCommand::JailGang { gang, days } => {
                let name = gang_name(self, gang)?;
                let members = self.comp::<Gang>(gang).map(|g| g.members.clone()).unwrap_or_default();
                let jailed = members.into_iter().filter(|&m| self.god_jail(m, days).is_ok()).count();
                Ok((vec![gang], format!("jailed {jailed} of {name} for {days} days")))
            }
            PlayerCommand::FireAllGuards => {
                let guards = law_brain::guards(self);
                for &g in &guards {
                    // As `law::reconcile_guards` dismisses: no vacancy (job
                    // search would rehire the whole watch the next morning;
                    // reconcile_guards rehires up to the lever, five a day),
                    // but a Fire event, so it reaches the biography. An
                    // escort in progress ends: the suspect walks.
                    let text = format!("{} dismissed from the guard", self.name_of(g));
                    crate::systems::economy::dismiss(self, g, None, text);
                }
                law_brain::recompute_captain(self);
                Ok((guards, "dismissed every guard".to_string()))
            }
            PlayerCommand::SetTreasury(coins) => {
                let t = self.treasury_mut().ok_or("SetTreasury: no Civic Hall")?;
                t.coins = coins;
                Ok((vec![], format!("set the Treasury to {coins}")))
            }
            PlayerCommand::Riot(_)
            | PlayerCommand::Litter { .. }
            | PlayerCommand::SplitGang(_)
            | PlayerCommand::Derelict(_)
            | PlayerCommand::BuyBuilding { .. } => self.cmd_god_district(cmd),
            PlayerCommand::GrantAsset { agent, kind, tier } => {
                let a =
                    crate::systems::assets::grant(self, agent, kind, tier).map_err(|e| format!("GrantAsset: {e}"))?;
                Ok((vec![agent, a], format!("granted {} a {}", self.name_of(agent), self.name_of(a))))
            }
            PlayerCommand::FloodGuns { gang, n, tier } => {
                let made =
                    crate::systems::weapons::flood(self, gang, n, tier).map_err(|e| format!("FloodGuns: {e}"))?;
                Ok((
                    vec![gang],
                    format!("flooded {}'s armoury with {made} T{tier} weapons", self.owner_label(Some(gang))),
                ))
            }
            PlayerCommand::Wreck(a) => {
                if !self.has::<crate::components::Asset>(a) {
                    return Err("Wreck: no such asset".into());
                }
                let what = self.name_of(a);
                crate::systems::assets::wreck(self, a, "by god");
                Ok((vec![a], format!("wrecked the {what}")))
            }
            PlayerCommand::ChromeEveryone { .. }
            | PlayerCommand::FloodStims { .. }
            | PlayerCommand::Brick(_)
            | PlayerCommand::Chase(_) => self.cmd_god_assets(cmd),
            PlayerCommand::WipeData(_)
            | PlayerCommand::SetTech { .. }
            | PlayerCommand::GrantData { .. }
            | PlayerCommand::GrantDeck { .. }
            | PlayerCommand::SetIce { .. }
            | PlayerCommand::Fry(_)
            | PlayerCommand::RunNow { .. }
            | PlayerCommand::WipeCorpData(_)
            | PlayerCommand::GrantDecks { .. }
            | PlayerCommand::SetCorpIce { .. } => self.cmd_god_virt(cmd),
            PlayerCommand::PlantRumour { .. }
            | PlayerCommand::SetReputation { .. }
            | PlayerCommand::GrantSkill { .. }
            | PlayerCommand::SetCreed { .. }
            | PlayerCommand::DeclareVendetta { .. }
            | PlayerCommand::KillFriend { .. }
            | PlayerCommand::Hunt { .. } => self.cmd_god_word(cmd),
            _ => self.cmd_god_corp(cmd),
        }
    }

    /// The M15 god commands (plan W42, phase 1).
    fn cmd_god_word(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        match *cmd {
            PlayerCommand::PlantRumour { about, deed, object, district, reach } => {
                if !self.has::<crate::components::Identity>(about)
                    && !self.has::<Gang>(about)
                    && !self.has::<Corp>(about)
                {
                    return Err("PlantRumour: no such agent or faction".into());
                }
                if district.index() >= self.rumours.len() {
                    return Err(format!("PlantRumour: no district {}", district.index()));
                }
                let reach = reach.clamp(0.0, 1.0);
                let e = crate::word::PoolEntry {
                    deed,
                    actor: Some(about),
                    object,
                    tick: self.tick,
                    hops: 0,
                    reach,
                    hole: None,
                    story: None,
                    kin: Default::default(),
                    told: Default::default(),
                    district,
                    press: 0,
                };
                crate::systems::gossip::post(self, district, e);
                // One first-hand holder: the living adult of the district nearest `about`.
                let from = self.comp::<crate::components::Position>(about).map(|p| p.tile);
                let holder = self
                    .citizens()
                    .into_iter()
                    .filter(|&a| a != about && crate::systems::demography::is_adult(self, a))
                    .filter(|&a| self.has::<crate::components::Memory>(a))
                    .filter(|&a| crate::systems::gossip::home_district(self, a) == Some(district))
                    .min_by_key(|&a| {
                        let d = match (from, self.comp::<crate::components::Position>(a)) {
                            (Some(f), Some(p)) => f.manhattan(p.tile),
                            _ => u32::MAX,
                        };
                        (d, a)
                    });
                if let Some(h) = holder {
                    let sal = self.config.gossip.deed_sal.get(deed);
                    let sev = self.config.gossip.deed_sev.get(deed);
                    let entry = crate::components::MemoryEntry {
                        subject: Some(about),
                        salience: sal,
                        valence: -sev * sal,
                        deed: Some(deed),
                        object,
                        ..crate::components::MemoryEntry::blank(crate::components::MemoryKind::Rumour, self.tick)
                    };
                    crate::systems::memory::hear_entry(self, h, entry);
                }
                let what = object.map(|o| format!(" {}", self.name_of(o))).unwrap_or_default();
                let text = format!(
                    "planted \"{} {}{what}\" in {} at reach {reach:.2}",
                    self.name_of(about),
                    deed.label(),
                    self.district_name(district)
                );
                Ok((vec![about], text))
            }
            PlayerCommand::GrantSkill { agent, skill, value, suit } => {
                if !crate::systems::law::living(self, agent) || !self.has::<crate::components::Skills>(agent) {
                    return Err("GrantSkill: no such living agent".into());
                }
                let v = value.clamp(0.0, 1.0);
                if let Some(s) = self.comp_mut::<crate::components::Skills>(agent) {
                    *s.social_mut(skill) = v;
                }
                if suit {
                    let mut a = crate::systems::reputation::appearance_of(self, agent);
                    a.dress = 3;
                    a.dress_pin = Some(3);
                    self.insert(agent, a);
                }
                let dress = if suit { " (in a suit)" } else { "" };
                Ok((vec![agent], format!("set {}'s {} to {v:.2}{dress}", self.name_of(agent), skill.label())))
            }
            PlayerCommand::SetCreed { gang, creed } => {
                let Some(g) = self.comp_mut::<Gang>(gang) else { return Err("SetCreed: no such gang".into()) };
                g.creed = creed;
                let name = g.name.clone();
                crate::systems::creeds::enforce(self, gang);
                let what = creed.map_or("no creed", |c| c.label());
                Ok((vec![gang], format!("{name} now holds {what}")))
            }
            PlayerCommand::SetReputation { who, axis, value, days } => {
                let i = who.index as usize;
                if i >= self.reputation.len() || !self.is_alive(who) {
                    return Err("SetReputation: no such agent or faction".into());
                }
                let until = self.tick + u64::from(days) * crate::time::TICKS_PER_DAY;
                let r = self.reputation[i].get_or_insert_with(Default::default);
                let mut axes = r.pinned.filter(|&(_, t)| t > self.tick).map_or_else(|| r.axes(), |(a, _)| a);
                axes[axis.index()] = value.clamp(0.0, 1.0);
                r.set_axes(axes);
                r.pinned = Some((axes, until));
                Ok((
                    vec![who],
                    format!("pinned {}'s {} at {value:.2} for {days} days", self.name_of(who), axis.label()),
                ))
            }
            PlayerCommand::DeclareVendetta { a, b, weight } => {
                let is_faction =
                    |w: &World, f: EntityId| w.has::<Gang>(f) || w.has::<Corp>(f) || w.has::<crate::components::Law>(f);
                if a == b || !is_faction(self, a) || !is_faction(self, b) {
                    return Err("DeclareVendetta: two different factions".into());
                }
                let w = weight.clamp(0.0, 1.0);
                for x in [a, b] {
                    if faction_leader(self, x).is_none() {
                        return Err(format!(
                            "DeclareVendetta: {} has no leader",
                            crate::systems::grudges::label(self, x)
                        ));
                    }
                }
                // The feud is declared open and held for `declared_days`.
                crate::systems::grudges::declare(self, a, b, w);
                let text = format!(
                    "declared a vendetta between {} and {} ({w:.2})",
                    crate::systems::grudges::label(self, a),
                    crate::systems::grudges::label(self, b)
                );
                Ok((vec![a, b], text))
            }
            PlayerCommand::KillFriend { of, by } => {
                if !crate::systems::law::living(self, of) || !crate::systems::law::living(self, by) || of == by {
                    return Err("KillFriend: no such living agents".into());
                }
                let friend = self
                    .neighbours(of)
                    .filter(|&o| o != by && crate::systems::law::living(self, o))
                    .filter_map(|o| {
                        self.edge(of, o)
                            .filter(|e| e.kind == crate::components::RelKind::Friend)
                            .map(|e| (o, e.affinity))
                    })
                    .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)))
                    .map(|(o, _)| o);
                let Some(victim) = friend else { return Err("KillFriend: no living Friend".into()) };
                let tile = self.comp::<crate::components::Position>(victim).map(|p| p.tile).unwrap_or_default();
                let d = self.district_of(tile);
                self.kill_by(victim, crate::components::DeathCause::Violence, Some(by));
                // The natural killing's path: witnesses rolled at the death's
                // tile, the district's crime note, a guard's report, the
                // pool post naming the killer when anyone noticed.
                let crime = crate::components::Crime::Murder;
                crate::systems::law::raise_crime_on(self, by, None, Some(victim), crime, tile);
                let named = self.rumours.iter().any(|p| {
                    p.entries
                        .iter()
                        .any(|e| e.deed == crate::word::Deed::Killed && e.object == Some(victim) && e.actor == Some(by))
                });
                // Nobody noticed: one witness, as the binder picks one (a
                // living adult whose Home is in the death's district, on the
                // Hunt stream), so the killing is named.
                let pool: Vec<EntityId> = self
                    .citizens()
                    .into_iter()
                    .filter(|&w| w != by && w != of && crate::systems::law::living(self, w))
                    .filter(|&w| crate::systems::demography::is_adult(self, w))
                    .filter(|&w| crate::systems::gossip::home_district(self, w) == Some(d))
                    .collect();
                let mut rng = self.rng.word(crate::word::WordNs::Hunt, self.tick, u64::from(victim.index));
                let witness =
                    (!named && !pool.is_empty()).then(|| pool[rand::Rng::random_range(&mut rng, 0..pool.len())]);
                if let Some(w) = witness {
                    self.remember_crime(w, by, crime, crate::systems::law::crime_salience(crime), Some(victim));
                    crate::systems::gossip::name_actor(self, victim, by);
                    crate::systems::gossip::post_deed(self, d, crate::word::Deed::Killed, Some(by), Some(victim));
                }
                let text = format!(
                    "{} killed {}, {}'s friend{}",
                    self.name_of(by),
                    self.name_of(victim),
                    self.name_of(of),
                    witness.map(|w| format!(" (seen by {})", self.name_of(w))).unwrap_or_default()
                );
                Ok((vec![by, victim, of], text))
            }
            PlayerCommand::Hunt { hunter, target } => {
                crate::systems::hunt::god_hunt(self, hunter, target)?;
                Ok((vec![hunter, target], format!("{} hunts {}", self.name_of(hunter), self.name_of(target))))
            }
            _ => Err("not a word command".into()),
        }
    }
}

/// M15 W42: a faction's leader: a gang's leader, a corp's exec, the captain.
pub fn faction_leader(world: &World, f: EntityId) -> Option<EntityId> {
    if let Some(g) = world.comp::<Gang>(f) {
        return g.leader.filter(|&l| crate::systems::law::living(world, l));
    }
    if let Some(c) = world.comp::<Corp>(f) {
        return c.exec.filter(|&e| crate::systems::law::living(world, e));
    }
    world.comp::<crate::components::Law>(f).and_then(|l| l.captain).filter(|&c| crate::systems::law::living(world, c))
}

impl World {
    /// The M14 god commands on the plane (plan V42, phase 1).
    fn cmd_god_virt(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{tech, virt};
        match *cmd {
            PlayerCommand::WipeData(b) => {
                if !self.has::<Building>(b) {
                    return Err("WipeData: no such building".into());
                }
                virt::relink(self);
                let n = virt::node_of_building(self, b).ok_or("WipeData: the building has no node")?;
                let units = tech::wipe_store(self, n, None);
                Ok((vec![b], format!("wiped {units} Data at {}", self.name_of(b))))
            }
            PlayerCommand::SetTech { corp, track, tier } => {
                if !self.has::<Corp>(corp) {
                    return Err("SetTech: no such corp".into());
                }
                tech::set_tier(self, corp, track, tier);
                let now = self.comp::<Corp>(corp).map_or(0, |c| c.tech.tier_of(track));
                Ok((vec![corp], format!("set {}'s {track} to {now}", self.owner_label(Some(corp)))))
            }
            PlayerCommand::GrantData { faction, track, units } => {
                tech::grant_data(self, faction, track, units).map_err(|e| format!("GrantData: {e}"))?;
                Ok((vec![faction], format!("gave {} {units} {track} Data", self.owner_label(Some(faction)))))
            }
            PlayerCommand::GrantDeck { agent, tier } => {
                if !self.has::<Brain>(agent) {
                    return Err("GrantDeck: no such agent".into());
                }
                let a = crate::systems::assets::grant(self, agent, crate::components::AssetKind::Deck, tier)
                    .map_err(|e| format!("GrantDeck: {e}"))?;
                Ok((vec![agent, a], format!("granted {} a tier-{tier} Deck", self.name_of(agent))))
            }
            PlayerCommand::SetIce { building, tier } => {
                virt::relink(self);
                let n = virt::node_of_building(self, building).ok_or("SetIce: the building has no node")?;
                let tier = tier.min(3);
                let p = virt::profile_mut(self, n).ok_or("SetIce: the node has no ICE")?;
                p.ice = tier;
                p.ice_maker = None;
                p.ice_arrears = 0;
                virt::bump_epoch(self);
                Ok((vec![building], format!("set {} to ICE {tier}", self.name_of(building))))
            }
            PlayerCommand::Fry(agent) => {
                let text = virt::god_fry(self, agent).map_err(|e| format!("Fry: {e}"))?;
                Ok((vec![agent], text))
            }
            PlayerCommand::RunNow { agent, target, purpose } => {
                let text = virt::god_run_now(self, agent, target, purpose).map_err(|e| format!("RunNow: {e}"))?;
                Ok((vec![agent, target], text))
            }
            PlayerCommand::WipeCorpData(corp) => {
                if !self.has::<Corp>(corp) {
                    return Err("WipeCorpData: no such corp".into());
                }
                virt::relink(self);
                let nodes: Vec<crate::virt::NodeId> = (0..self.virt.nodes.len())
                    .map(|i| crate::virt::NodeId(i as u16))
                    .filter(|&n| self.virt.node(n).is_some_and(|x| x.alive && x.owner == Some(corp)))
                    .collect();
                let units: u32 = nodes.into_iter().map(|n| tech::wipe_store(self, n, None)).sum();
                Ok((vec![corp], format!("wiped {units} Data across {}", self.owner_label(Some(corp)))))
            }
            PlayerCommand::GrantDecks { district, n, tier } => {
                let mut picks: Vec<(f32, EntityId)> = self
                    .citizens()
                    .into_iter()
                    .filter(|&a| crate::systems::demography::is_adult(self, a))
                    .filter(|&a| self.comp::<crate::components::Kit>(a).is_none_or(|k| k.deck.is_none()))
                    .filter(|&a| {
                        self.comp::<crate::components::Household>(a)
                            .and_then(|h| h.home)
                            .is_some_and(|h| self.district_of_building(h) == district)
                    })
                    .map(|a| (self.comp::<crate::components::Skills>(a).map_or(0.0, |s| s.hacking), a))
                    .collect();
                picks.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                let mut given = Vec::new();
                for (_, a) in picks.into_iter().take(usize::from(n)) {
                    if crate::systems::assets::grant(self, a, crate::components::AssetKind::Deck, tier).is_ok() {
                        given.push(a);
                    }
                }
                if given.is_empty() {
                    return Err(format!("GrantDecks: no adult without a deck lives in district {}", district.index()));
                }
                let text =
                    format!("granted {} adults of district {} a tier-{tier} Deck", given.len(), district.index());
                Ok((given, text))
            }
            PlayerCommand::SetCorpIce { corp, tier } => {
                if !self.has::<Corp>(corp) {
                    return Err("SetCorpIce: no such corp".into());
                }
                virt::relink(self);
                let tier = tier.min(3);
                let nodes: Vec<crate::virt::NodeId> = (0..self.virt.nodes.len())
                    .map(|i| crate::virt::NodeId(i as u16))
                    .filter(|&n| self.virt.node(n).is_some_and(|x| x.alive && x.owner == Some(corp)))
                    .collect();
                let mut set = 0;
                for n in nodes {
                    if let Some(p) = virt::profile_mut(self, n) {
                        p.ice = tier;
                        p.ice_maker = None;
                        p.ice_arrears = 0;
                        set += 1;
                    }
                }
                virt::bump_epoch(self);
                Ok((vec![corp], format!("set {set} of {}'s nodes to ICE {tier}", self.owner_label(Some(corp)))))
            }
            _ => Err("not a Virt god command".into()),
        }
    }

    /// The M13 god commands on assets (plan D48, docs/history/GOD_SCENARIOS_V4.md).
    fn cmd_god_assets(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::components::{Asset, AssetKind, AssetLoc, Controller, Good, Slot};
        use crate::systems::{assets, demography, law, stims};
        if !self.config.assets.enabled {
            return Err("assets are off".into());
        }
        match *cmd {
            PlayerCommand::ChromeEveryone { tier } => {
                if assets::list_price(self, AssetKind::Implant(Slot::Arms), tier).is_none() {
                    return Err(format!("ChromeEveryone: no implant at tier {tier}"));
                }
                let adults: Vec<EntityId> = self
                    .citizens()
                    .into_iter()
                    .filter(|&a| law::living(self, a) && demography::is_adult(self, a))
                    .collect();
                let mut n = 0;
                for &a in &adults {
                    for slot in [Slot::Arms, Slot::Nerves] {
                        if assets::grant(self, a, AssetKind::Implant(slot), tier).is_ok() {
                            n += 1;
                        }
                    }
                }
                Ok((vec![], format!("chromed {} adults ({n} T{tier} implants)", adults.len())))
            }
            PlayerCommand::FloodStims { district, n } => {
                if district.index() >= self.districts.len() {
                    return Err("FloodStims: no such district".into());
                }
                let name = self.district_name(district).to_string();
                let control = self.districts[district.index()].control;
                let mut hideouts: Vec<(EntityId, EntityId)> = Vec::new();
                for g in self.gangs() {
                    let Some(h) = self.hideout_of(g) else { continue };
                    let deals_here = stims::deal_bar(self, g).is_some_and(|b| self.district_of_building(b) == district);
                    if deals_here || control == Controller::Gang(g) || self.district_of_building(h) == district {
                        hideouts.push((g, h));
                    }
                }
                if hideouts.is_empty() {
                    return Err(format!("FloodStims: no gang deals in {name}"));
                }
                let mut added = 0;
                for &(_, h) in &hideouts {
                    added += self.add_stock(h, Good::Stims, n);
                }
                let gangs: Vec<EntityId> = hideouts.iter().map(|&(g, _)| g).collect();
                Ok((gangs, format!("flooded {} Hideouts dealing in {name} with {added} Stims", hideouts.len())))
            }
            PlayerCommand::Brick(lender) => {
                if !(self.has::<Brain>(lender) || self.has::<Gang>(lender) || self.has::<Corp>(lender)) {
                    return Err("Brick: no such lender".into());
                }
                let repo_days = self.config.assets.repo_days;
                let mut bodies = Vec::new();
                let mut n = 0;
                for a in assets::all_assets(self) {
                    let Some(x) = self.comp_mut::<Asset>(a) else { continue };
                    if !x.kind.is_implant() {
                        continue;
                    }
                    let Some(f) = x.finance.as_mut() else { continue };
                    if f.lender != Some(lender) {
                        continue;
                    }
                    f.arrears = f.arrears.max(repo_days);
                    x.bricked = true;
                    n += 1;
                    if let AssetLoc::Installed(b) = x.loc {
                        bodies.push(b);
                    }
                }
                bodies.sort_unstable();
                bodies.dedup();
                for &b in &bodies {
                    assets::rekit(self, b);
                }
                self.stats.current.repos += n;
                let who = self.owner_label(Some(lender));
                Ok((vec![lender], format!("{who} bricked {n} financed implants in {} bodies", bodies.len())))
            }
            PlayerCommand::Chase(agent) => {
                if !law::living(self, agent) || !self.has::<Brain>(agent) {
                    return Err("Chase: no such agent".into());
                }
                self.chase_pins.insert(agent);
                Ok((vec![agent], format!("pinned a chase on {}'s next trip", self.name_of(agent))))
            }
            _ => Err("not an asset god command".into()),
        }
    }

    /// The M12 god commands on districts (docs/history/GOD_SCENARIOS_V3.md, plan D42).
    fn cmd_god_district(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{gang, litter, ownership, riot, street};
        let check_district = |w: &World, d: crate::components::DistrictId, what: &str| {
            if d.index() < w.districts.len() {
                Ok(w.district_name(d).to_string())
            } else {
                Err(format!("{what}: no such district"))
            }
        };
        match *cmd {
            PlayerCommand::Riot(d) => {
                let name = check_district(self, d, "Riot")?;
                riot::start(self, d, true).map_err(|e| format!("Riot in {name}: {e}"))?;
                Ok((vec![], format!("raised a riot in {name}")))
            }
            PlayerCommand::Litter { district, level } => {
                let name = check_district(self, district, "Litter")?;
                if self.litter.is_empty() {
                    return Err("Litter: litter is off".into());
                }
                let v = (level.clamp(0.0, 1.0) * 254.0).round() as u8;
                let tiles = self.districts[district.index()].streets.clone();
                for &i in &tiles {
                    if let Some(t) = self.litter.get_mut(i as usize) {
                        // Rubble (the Damage hook) stays rubble.
                        if *t != litter::RUBBLE {
                            *t = v;
                        }
                    }
                }
                litter::district_means(self);
                Ok((vec![], format!("littered {} street tiles of {name} to {v}", tiles.len())))
            }
            PlayerCommand::SplitGang(g) => {
                let name = self.comp::<Gang>(g).map(|x| x.name.clone()).ok_or("SplitGang: no such gang")?;
                let s = gang::split(self, g, None, false).map_err(|e| format!("SplitGang {name}: {e}"))?;
                let sn = self.comp::<Gang>(s).map_or_else(String::new, |x| x.name.clone());
                Ok((vec![g, s], format!("split {name}: the {sn}")))
            }
            PlayerCommand::Derelict(b) => {
                let what = self.name_of(b);
                let Some(bd) = self.comp::<Building>(b) else { return Err("Derelict: no such building".into()) };
                if bd.demolished || bd.derelict {
                    return Err(format!("Derelict: {what} is already gone or derelict"));
                }
                if !street::can_go_derelict(bd.kind) {
                    return Err(format!("Derelict: a {} cannot go derelict", bd.kind.label()));
                }
                if !street::make_derelict(self, b, "by god") {
                    return Err(format!("Derelict: {what} refused"));
                }
                Ok((vec![b], format!("made {what} derelict")))
            }
            PlayerCommand::BuyBuilding { buyer, building, price } => {
                let Some(bd) = self.comp::<Building>(building) else {
                    return Err("BuyBuilding: no such building".into());
                };
                if bd.demolished {
                    return Err("BuyBuilding: demolished".into());
                }
                let (owner, derelict) = (bd.owner, bd.derelict);
                if buyer.is_some_and(|t| !(self.has::<Wallet>(t) || self.has::<Gang>(t) || self.has::<Corp>(t))) {
                    return Err("BuyBuilding: the buyer is not an agent, gang or corp".into());
                }
                if owner == buyer && !derelict {
                    return Err("BuyBuilding: the buyer owns it".into());
                }
                let price = price.max(0);
                // As every M11 flow: the buyer pays, the owner (or the city) is paid.
                let paid = if owner == buyer {
                    0
                } else {
                    ownership::charge(self, buyer, owner, price, ownership::Flow::Sale)
                };
                let what = self.name_of(building);
                let (from, to) = (self.owner_label(owner), self.owner_label(buyer));
                if derelict {
                    street::restore(self, building, buyer, "bought by god");
                } else {
                    crate::systems::corps::move_building(self, building, buyer);
                    if buyer.is_none() {
                        crate::systems::corps::end_contract(self, building, "bought by the city");
                    }
                }
                if let Some(c) = owner.filter(|&o| self.has::<Corp>(o) && Some(o) != buyer) {
                    ownership::push_corp_shock(self, c, CorpShock::BuildingLost);
                }
                let actors: Vec<EntityId> = [Some(building), owner, buyer].into_iter().flatten().collect();
                Ok((actors, format!("{to} bought {what} from {from} for {paid}")))
            }
            _ => Err("not a god command".into()),
        }
    }

    /// The M11 god commands on corps (docs/history/GOD_SCENARIOS_V2.md).
    fn cmd_god_corp(&mut self, cmd: &PlayerCommand) -> Result<(Vec<EntityId>, String), String> {
        use crate::systems::{classes, corps, ownership};
        let corp_name = |w: &World, c: EntityId| w.comp::<Corp>(c).map(|c| c.name.clone()).ok_or("no such corp");
        match *cmd {
            PlayerCommand::FundCorp { corp, amount } => {
                let name = corp_name(self, corp)?;
                if let Some(c) = self.comp_mut::<Corp>(corp) {
                    c.treasury += amount;
                }
                Ok((vec![corp], format!("gave {name} {amount} coins")))
            }
            PlayerCommand::BankruptCorp(corp) => {
                let name = corp_name(self, corp)?;
                let days = self.config.corps.bankrupt_days + 1;
                let back = self.tick.saturating_sub(days * crate::time::TICKS_PER_DAY);
                if let Some(c) = self.comp_mut::<Corp>(corp) {
                    c.treasury = -1;
                    c.negative_since = Some(back);
                }
                corps::bankrupt(self, corp);
                Ok((vec![corp], format!("bankrupted {name}")))
            }
            PlayerCommand::SeizeBuildings { owner, to } => {
                if to.is_some_and(|t| !(self.has::<Wallet>(t) || self.has::<Gang>(t) || self.has::<Corp>(t))) {
                    return Err("SeizeBuildings: the new owner is not an agent, gang or corp".into());
                }
                if owner == to {
                    return Err("SeizeBuildings: same owner".into());
                }
                let owned: Vec<EntityId> = self
                    .with::<Building>()
                    .into_iter()
                    .filter(|&b| self.comp::<Building>(b).is_some_and(|bd| bd.owner == owner && !bd.demolished))
                    .collect();
                if owned.is_empty() {
                    return Err(format!("SeizeBuildings: {} owns nothing", self.owner_label(owner)));
                }
                let (from_label, to_label) = (self.owner_label(owner), self.owner_label(to));
                for &b in &owned {
                    corps::move_building(self, b, to);
                    if to.is_none() {
                        // The city does not keep a corp's guard contract.
                        corps::end_contract(self, b, "seized by the city");
                    }
                }
                if let Some(c) = owner.filter(|&o| self.has::<Corp>(o)) {
                    ownership::push_corp_shock(self, c, CorpShock::BuildingLost);
                }
                let actors: Vec<EntityId> = owner.into_iter().chain(to).collect();
                Ok((actors, format!("gave all {} buildings of {from_label} to {to_label}", owned.len())))
            }
            PlayerCommand::KillExec(corp) => {
                let name = corp_name(self, corp)?;
                let exec = self.comp::<Corp>(corp).and_then(|c| c.exec);
                let exec = exec.filter(|&e| self.is_alive(e) && self.has::<Brain>(e));
                let exec = exec.ok_or_else(|| format!("KillExec: {name} has no living exec"))?;
                let who = self.name_of(exec);
                self.kill_by(exec, DeathCause::Violence, None);
                Ok((vec![corp, exec], format!("struck {who}, exec of {name}, dead")))
            }
            PlayerCommand::KillStaff(corp) => {
                let name = corp_name(self, corp)?;
                let staff = self.comp::<Corp>(corp).map(|c| classes::employees(self, c)).unwrap_or_default();
                for &a in &staff {
                    self.kill_by(a, DeathCause::Violence, None);
                }
                Ok((vec![corp], format!("killed all {} employees of {name}", staff.len())))
            }
            PlayerCommand::SetCorpOrder { corp, order, niche, days } => {
                let name = corp_name(self, corp)?;
                let (now, evict_days) = (self.tick, self.config.rent.evict_days);
                let c = self.comp_mut::<Corp>(corp).ok_or("no such corp")?;
                let niche = match niche {
                    Some(n) if !c.niches.contains(&n) => return Err(format!("SetCorpOrder: {name} is not in {n}")),
                    Some(n) => n,
                    None => c
                        .order_niche
                        .filter(|n| c.niches.contains(n))
                        .or_else(|| c.niches.iter().next().copied())
                        .ok_or("SetCorpOrder: the corp has no niche")?,
                };
                // As `corp_brain::rescore` switches (D22).
                if c.order == CorpOrder::Squeeze {
                    c.wage_mult = 1.0;
                    c.evict_days_override = None;
                }
                if order == CorpOrder::Squeeze {
                    match niche {
                        Niche::Housing => c.evict_days_override = Some(evict_days.saturating_sub(1).max(2)),
                        Niche::Food => c.wage_mult = 0.9,
                        Niche::Security | Niche::Tech | Niche::Vice => {}
                    }
                }
                c.order = order;
                c.order_niche = Some(niche);
                c.order_since = now;
                c.pinned_until = Some(now + u64::from(days) * crate::time::TICKS_PER_DAY);
                Ok((vec![corp], format!("pinned {name} to {order} in {niche} for {days} days")))
            }
            PlayerCommand::StrikeNow(corp) => {
                let name = corp_name(self, corp)?;
                let strikers = self.comp::<Corp>(corp).map(|c| classes::employees(self, c)).unwrap_or_default();
                if strikers.is_empty() {
                    return Err(format!("StrikeNow: {name} has no workers"));
                }
                classes::walk_out(self, &strikers);
                self.stats.current.strikes += 1;
                let text = format!("{} workers of {name} walk out (by god)", strikers.len());
                self.push_event(EventKind::Strike, &[corp], text);
                crate::systems::corp_brain::push_shock(self, corp, CorpShock::Strike);
                Ok((vec![corp], format!("called a strike at {name}")))
            }
            PlayerCommand::WipeTreasuries => {
                let all = self.corps();
                for &c in &all {
                    if let Some(cc) = self.comp_mut::<Corp>(c) {
                        cc.treasury = 0;
                    }
                }
                let n = all.len();
                Ok((all, format!("wiped the treasuries of {n} corps")))
            }
            _ => Err("not a god command".into()),
        }
    }

    /// A god sentence: no report, no capacity check. A prisoner's sentence is
    /// extended instead. It is filed as Assault: a full Jail makes room by
    /// freeing the Theft prisoner with the longest sentence, which a long
    /// god sentence for Theft always was.
    fn god_jail(&mut self, who: EntityId, days: u32) -> Result<(), String> {
        if !self.is_alive(who) || !self.has::<Brain>(who) {
            return Err("JailAgent: no such agent".into());
        }
        let until = self.tick + u64::from(days.max(1)) * crate::time::TICKS_PER_DAY;
        // Already inside: the sentence runs to whichever end is later.
        if let Some(s) = self.comp_mut::<Sentence>(who) {
            s.until_tick = s.until_tick.max(until);
            s.crime = s.crime.max(Crime::Assault);
            return Ok(());
        }
        let jail = self.building_of_kind(BuildingKind::Jail).ok_or("JailAgent: no Precinct")?;
        crate::systems::law::sentence(self, who, Crime::Assault, until, jail);
        Ok(())
    }
}
