use super::*;

impl World {
    /// A lever's outcome as an event (Real economy phase 1).
    fn lever_result(&mut self, r: Result<String, String>) {
        match r {
            Ok(text) => self.push_event(EventKind::PlayerAction, &[], text),
            Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], e),
        };
    }

    /// Queue a command for the start of the next tick.
    pub fn push_command(&mut self, cmd: PlayerCommand) {
        self.command_queue.push(cmd);
    }

    /// Move every command out of `cmds` into the queue, in order.
    pub fn push_commands(&mut self, cmds: &mut Vec<PlayerCommand>) {
        self.command_queue.append(cmds);
    }

    /// Drain the queue, apply each command in order, log it.
    pub fn apply_commands(&mut self) {
        let queue = std::mem::take(&mut self.command_queue);
        for cmd in queue {
            self.apply_command(&cmd);
            self.command_log.push((self.tick, cmd));
        }
    }

    fn apply_command(&mut self, cmd: &PlayerCommand) {
        match cmd {
            PlayerCommand::Bind(hole) => {
                crate::systems::bind::bind(self, *hole);
            }
            PlayerCommand::ReleaseReserve { amount } => self.cmd_release_reserve(*amount),
            PlayerCommand::SetTaxRate(r) => {
                // Review finding 1: down to the surplus credit's floor (the
                // band's own reach), so a set rate can hold a credit.
                let (lo, hi) = tax_lever_range(&self.config);
                self.levers.tax_rate = r.clamp(lo, hi);
                // Real economy E21: a set rate pins against the band (written
                // only while the band runs: the state is the milestone's).
                if crate::systems::treasury::on(self) {
                    self.econ.tax_pinned = true;
                }
                self.push_event(EventKind::PlayerAction, &[], format!("Tax rate set to {:.2}", self.levers.tax_rate));
            }
            PlayerCommand::SetSentenceMult(m) => {
                self.levers.sentence_mult = m.clamp(0.5, 3.0);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Sentence multiplier set to {:.2}", self.levers.sentence_mult),
                );
            }
            PlayerCommand::SetGuardCount(n) => {
                // Scale S4: the ceiling per capita (60 at 2,000).
                self.levers.guard_count = (*n).min(self.config.lever_ceiling(60));
                // Jobs v2 J17: a set headcount pins against the civic budget.
                if crate::systems::treasury::on(self) {
                    self.econ.civic_pinned = true;
                }
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Guard count set to {}", self.levers.guard_count),
                );
            }
            PlayerCommand::SetImmigrationPerWeek(n) => {
                self.levers.immigration_per_week = (*n).min(self.config.lever_ceiling(30));
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Immigration set to {} per week", self.levers.immigration_per_week),
                );
            }
            PlayerCommand::SetDolePerDay(n) => {
                self.levers.dole_per_day = (*n).min(10);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Dole set to {} per day", self.levers.dole_per_day),
                );
            }
            PlayerCommand::GrantCoins { agent, amount } => self.cmd_grant_coins(*agent, *amount),
            PlayerCommand::Pin(agent, pinned) => {
                if let Some(brain) = self.comp_mut::<Brain>(*agent) {
                    brain.pinned = *pinned;
                    let text = if *pinned { "Pinned" } else { "Unpinned" };
                    self.push_event(EventKind::PlayerAction, &[*agent], format!("{text} {}", self.name_of(*agent)));
                } else {
                    self.push_event(EventKind::PlayerActionFailed, &[*agent], "Pin: no such agent");
                }
            }
            PlayerCommand::Possess(agent, on) => crate::systems::player::possess(self, *agent, *on),
            PlayerCommand::Steps { agent, steps } => crate::systems::player::steps(self, *agent, steps),
            PlayerCommand::Goal { agent, goal } => crate::systems::player::goal(self, *agent, *goal),
            PlayerCommand::Stop(agent) => crate::systems::player::stop(self, *agent),
            PlayerCommand::SetAutopilot(agent, on) => crate::systems::player::set_autopilot(self, *agent, *on),
            PlayerCommand::Act { agent, verb } => crate::systems::player::act(self, *agent, *verb),
            PlayerCommand::Fight { agent, order } => crate::systems::combat::order(self, *agent, *order),
            PlayerCommand::SetView(rect) => {
                self.view_rect = *rect;
            }
            PlayerCommand::SetSpeed(_) => {}
            PlayerCommand::SetLawPosture(p) => {
                if let Some(l) = self.law_mut() {
                    l.pinned = *p;
                } else {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetLawPosture: no Precinct");
                    return;
                }
                crate::systems::law_brain::rescore(self, 0.0, "pinned");
                let text = match p {
                    Some(p) => format!("Law posture pinned to {p}"),
                    None => "Law posture handed back to the captain".to_string(),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetGuardWeight { district, weight } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetGuardWeight: no such district");
                    return;
                }
                self.levers.guard_weight[i] = weight.clamp(0.0, 5.0);
                let text =
                    format!("Guard weight in {} set to {:.2}", self.district_name(*district), weight.clamp(0.0, 5.0));
                self.push_event(EventKind::PlayerAction, &[], text);
                crate::systems::law_brain::redeal(self, "lever");
            }
            PlayerCommand::SetStance { district, stance } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetStance: no such district");
                    return;
                }
                if let Some(crate::components::Stance::Crackdown(g)) = stance {
                    if !self.has::<Gang>(*g) {
                        self.push_event(EventKind::PlayerActionFailed, &[], "SetStance: no such gang");
                        return;
                    }
                }
                self.levers.stance_pin[i] = *stance;
                let name = self.district_name(*district).to_string();
                let text = match stance {
                    Some(s) => {
                        format!("Stance in {name} pinned to {}", crate::systems::law_brain::stance_label(self, *s))
                    }
                    None => format!("Stance in {name} handed back to the captain"),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
                crate::systems::law_brain::redeal(self, "pinned");
            }
            PlayerCommand::SetSanitation(n) => {
                self.levers.sanitation_count = (*n).min(self.config.lever_ceiling(60));
                // Jobs v2 J17: a set headcount pins against the civic budget.
                if crate::systems::treasury::on(self) {
                    self.econ.civic_pinned = true;
                }
                let text = format!("Sanitation headcount set to {}", self.levers.sanitation_count);
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetCivicAuto => {
                let text = if crate::systems::treasury::on(self) {
                    self.econ.civic_pinned = false;
                    Ok(format!(
                        "Civic headcounts released to the budget (now {} guards, {} sweepers)",
                        self.levers.guard_count, self.levers.sanitation_count
                    ))
                } else {
                    Err("SetCivicAuto: no civic budget (the levy is enacted)".to_string())
                };
                self.lever_result(text);
            }
            PlayerCommand::SetSanitationWeight { district, weight } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetSanitationWeight: no such district");
                    return;
                }
                let w = weight.clamp(0.0, 5.0);
                self.levers.sanitation_weight[i] = w;
                let text = format!("Sanitation weight in {} set to {w:.2}", self.district_name(*district));
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetCurfew { district, on } => {
                let i = district.index();
                if i >= self.districts.len() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetCurfew: no such district");
                    return;
                }
                self.levers.curfew[i] = *on;
                self.districts[i].curfew = *on;
                let name = self.district_name(*district).to_string();
                let text = if *on { format!("Curfew in {name}") } else { format!("Curfew in {name} lifted") };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetStimsLegal(on) => {
                self.levers.stims_legal = *on;
                let text = if *on { "Stims legal: Markets stock and sell them" } else { "Stims banned again" };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetAssetTax { kind, rate } => {
                let r = rate.clamp(0.0, 5.0);
                if let Some(t) = self.levers.asset_tax.get_mut(kind.index()) {
                    *t = r;
                }
                self.push_event(EventKind::PlayerAction, &[], format!("Asset tax on {kind:?} set to {r:.2}"));
            }
            PlayerCommand::SetImpound(on) => {
                self.levers.impound = *on;
                let text = if *on { "The city impounds unregistered vehicles" } else { "The city stops impounding" };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetCityIce(n) => {
                self.levers.city_ice = (*n).min(3);
                crate::systems::virt::set_city_ice(self);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("City ICE set to {} (Treasury and Precinct)", self.levers.city_ice),
                );
            }
            PlayerCommand::SetDataTax(r) => {
                self.levers.data_tax = r.clamp(0.0, 1.0);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Data tax set to {:.0} %", self.levers.data_tax * 100.0),
                );
            }
            PlayerCommand::SetHackSentence { crime, days } => {
                let slot = match crime {
                    Crime::Intrusion => 0,
                    Crime::DataTheft => 1,
                    _ => {
                        self.push_event(
                            EventKind::PlayerActionFailed,
                            &[],
                            "SetHackSentence: only Intrusion and Data Theft".to_string(),
                        );
                        return;
                    }
                };
                let d = (*days).clamp(1, 365);
                self.levers.hack_sentence_days[slot] = Some(d);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Sentence for {} set to {d} days", crime.label()),
                );
            }
            PlayerCommand::SetPressLicence(on) => {
                self.levers.press_licence = *on;
                let text = if *on {
                    "Press licence granted: every Feed publishes"
                } else {
                    "Press licence revoked: only the Civic Wire publishes"
                };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetNewsTax(r) => {
                self.levers.news_tax = r.clamp(0.0, 1.0);
                self.push_event(EventKind::PlayerAction, &[], format!("News tax set to {:.2}", self.levers.news_tax));
            }
            PlayerCommand::CensorStories { faction, on } => {
                let known = self.has::<Gang>(*faction)
                    || self.has::<Corp>(*faction)
                    || self.has::<crate::components::Law>(*faction);
                if !known {
                    self.push_event(EventKind::PlayerActionFailed, &[], "CensorStories: no such faction".to_string());
                    return;
                }
                let name = if self.has::<crate::components::Law>(*faction) {
                    "the Law".to_string()
                } else {
                    self.owner_label(Some(*faction))
                };
                let text = if *on {
                    self.levers.censored.insert(*faction);
                    format!("The Civic Wire buries every story about {name}")
                } else {
                    self.levers.censored.remove(faction);
                    format!("The Civic Wire may run stories about {name} again")
                };
                self.push_event(EventKind::PlayerAction, &[*faction], text);
            }
            PlayerCommand::SetLeisureTax(rate) => {
                self.levers.leisure_tax = rate.clamp(0.0, 1.0);
                let text = format!("Leisure tax set to {:.0}%", self.levers.leisure_tax * 100.0);
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::BanGambling(on) => {
                self.levers.ban_gambling = *on;
                let text = if *on { "Gambling banned" } else { "Gambling legal again" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::CloseLeisure { district, days } => {
                let until = self.tick + u64::from(*days) * crate::time::TICKS_PER_DAY;
                let mut n = 0;
                for kind in BuildingKind::LEISURE {
                    for b in self.buildings_of_kind(kind).to_vec() {
                        if self.district_of_building(b) != *district {
                            continue;
                        }
                        if let Some(bd) = self.comp_mut::<crate::components::Building>(b) {
                            bd.closed_until = Some(bd.closed_until.unwrap_or(0).max(until));
                            n += 1;
                        }
                    }
                }
                let text = format!("God closed {n} leisure venues in district {} for {days} days", district.index());
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetFun { district, value } => {
                let v = value.clamp(0.0, 1.0);
                // scan-ok: a god command
                for id in self.citizens() {
                    let home_d = self
                        .comp::<crate::components::Household>(id)
                        .and_then(|h| h.home)
                        .map(|h| self.district_of_building(h))
                        .or_else(|| self.comp::<crate::components::Position>(id).map(|p| self.district_of(p.tile)));
                    if home_d == Some(*district) {
                        if let Some(n) = self.comp_mut::<crate::components::Needs>(id) {
                            n.fun = v;
                        }
                    }
                }
                let text = format!("God set fun to {v:.2} in district {}", district.index());
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetPublicWorks(on) => {
                self.levers.public_works = *on;
                let text = if *on { "Public works open" } else { "Public works closed" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetExport(on) => {
                self.levers.export_open = *on;
                let text = if *on { "Exports open to the World" } else { "Exports closed" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetExportPrice { good, price } => {
                let price = (*price).max(0);
                self.outside.export.price.insert(*good, price);
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("Export price of {} set to {price}", good.label()),
                );
            }
            // Transit T15.
            PlayerCommand::CutLine { line, days } => {
                let _ = match crate::systems::transit::cut_line(self, *line, *days) {
                    Ok(text) => self.push_event(EventKind::PlayerAction, &[], format!("God: {text}")),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("God: {e}")),
                };
            }
            PlayerCommand::CloseStop { stop, days } => {
                let _ = match crate::systems::transit::close_stop(self, *stop, *days) {
                    Ok(text) => self.push_event(EventKind::PlayerAction, &[], format!("God: {text}")),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("God: {e}")),
                };
            }
            // Real economy phase 1 (plan E47).
            PlayerCommand::SetAppetite { good, mult } => {
                let text = match crate::systems::world_market::book_mut(self, *good) {
                    Some(b) => {
                        b.appetite_pin = mult.map(|m| m.max(0.0));
                        if let Some(m) = b.appetite_pin {
                            b.appetite = m;
                        }
                        Ok(match mult {
                            Some(m) => format!("World appetite for {} pinned at {m:.2}", good.label()),
                            None => format!("World appetite for {} released to the walk", good.label()),
                        })
                    }
                    None => Err("SetAppetite: no World book (the market is off)".to_string()),
                };
                self.lever_result(text);
            }
            PlayerCommand::SetAmbientAppetite(mult) => {
                let text = crate::systems::outside::pin_appetite(self, *mult);
                self.lever_result(text);
            }
            PlayerCommand::Embargo { id, on } => {
                let text = crate::systems::outside::set_embargo(self, *id, *on);
                self.lever_result(text);
            }
            PlayerCommand::ForceReinforce { id, bodies } => {
                let text = crate::systems::parents::force_reinforce(self, *id, *bodies);
                self.lever_result(text);
            }
            PlayerCommand::SetOutsideOrder { id, order, days } => {
                let text = crate::systems::parents::set_order(self, *id, *order, *days);
                self.lever_result(text);
            }
            PlayerCommand::ForceUplinkRun { parent, purpose, patron } => {
                let text = crate::systems::parents::force_uplink_run(self, *parent, *purpose, *patron);
                self.lever_result(text);
            }
            PlayerCommand::KillParent(id) => {
                let text = crate::systems::parents::kill_parent(self, *id, "God");
                self.lever_result(text);
            }
            PlayerCommand::WipeParentTreasury(id) => {
                let text = crate::systems::parents::wipe_treasury(self, *id);
                self.lever_result(text);
            }
            PlayerCommand::SetParentEdge { parent, edge } => {
                let text = crate::systems::parents::set_edge(self, *parent, *edge);
                self.lever_result(text);
            }
            PlayerCommand::SanctionParent { parent, days, cut } => {
                let text = crate::systems::parents::god_sanction(self, *parent, *days, *cut);
                self.lever_result(text);
            }
            PlayerCommand::BanScrip(on) => {
                let text = crate::systems::parents::ban_scrip(self, *on);
                self.lever_result(text);
            }
            PlayerCommand::SendBrothers(gang) => {
                let text = crate::systems::parents::send_brothers(self, *gang);
                self.lever_result(text);
            }
            PlayerCommand::PostAbroad { buyer, on, asset, crew } => {
                let text = crate::systems::abroad::god_post(self, *buyer, *on, *asset, crew);
                self.lever_result(text);
            }
            PlayerCommand::BlackwallBreach { on, patron } => {
                let text = crate::systems::abroad::god_breach(self, *on, *patron);
                self.lever_result(text);
            }
            PlayerCommand::SetStateGuards(cap) => {
                let text = crate::systems::abroad::set_state_guards(self, *cap);
                self.lever_result(text);
            }
            PlayerCommand::FoundParent(corp) => {
                let text = crate::systems::parents::god_found_parent(self, *corp);
                self.lever_result(text);
            }
            PlayerCommand::SetTariff(t) => {
                self.outside.tariff = t.clamp(0.0, 1.0);
                let text = Ok(format!("Tariff on remittances set to {:.2}", self.outside.tariff));
                self.lever_result(text);
            }
            PlayerCommand::SetExportCap { good, cap } => {
                let text = match crate::systems::world_market::book_mut(self, *good) {
                    Some(b) => {
                        b.cap_pin = *cap;
                        Ok(match cap {
                            Some(c) => format!("World daily cap for {} pinned at {c}", good.label()),
                            None => format!("World daily cap for {} released", good.label()),
                        })
                    }
                    None => Err("SetExportCap: no World book (the market is off)".to_string()),
                };
                self.lever_result(text);
            }
            PlayerCommand::SetImportAsk { good, mult } => {
                let mult = mult.max(0.0);
                let text = match crate::systems::world_market::book_mut(self, *good) {
                    Some(b) => {
                        b.ask_mult = mult;
                        Ok(format!("World ask multiplier for {} set to {mult:.2}", good.label()))
                    }
                    None => Err("SetImportAsk: no World book (the market is off)".to_string()),
                };
                self.lever_result(text);
            }
            PlayerCommand::SetCustoms(rate) => {
                let rate = rate.clamp(0.0, 1.0);
                self.econ.customs_pin = Some(rate);
                self.push_event(EventKind::PlayerAction, &[], format!("Customs rate set to {rate:.2}"));
            }
            PlayerCommand::SetTaxAuto => {
                let text = if crate::systems::treasury::on(self) {
                    self.econ.tax_pinned = false;
                    Ok(format!("Tax rate released to the band (now {:.2})", self.levers.tax_rate))
                } else {
                    Err("SetTaxAuto: no tax band (the levy is enacted)".to_string())
                };
                self.lever_result(text);
            }
            PlayerCommand::SetPrisonPermit(_)
            | PlayerCommand::Hostage { .. }
            | PlayerCommand::Wound { .. }
            | PlayerCommand::Subscribe { .. }
            | PlayerCommand::TagAgent { .. }
            | PlayerCommand::Coerce { .. }
            | PlayerCommand::CallVote(_)
            | PlayerCommand::ExposeSecret(_) => {
                let r = self.apply_m16b(cmd);
                self.lever_result(r);
            }
            PlayerCommand::SetFixerLicence(on) => {
                self.levers.fixer_licence = *on;
                // C39: off floors every office's heat at 0.5 now (`heat_add`
                // clamps to the floor); on lifts the floor.
                for f in self.buildings_of_kind(BuildingKind::Fixer).to_vec() {
                    crate::systems::contracts::heat_add(self, f, 0.0);
                }
                let text = if *on {
                    "Lever: Fixers licensed again"
                } else {
                    "Lever: Fixers unlicensed: every office's heat floored at 0.5, no new office registers"
                };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetAccessoryMult(m) => {
                if !m.is_finite() {
                    self.push_event(EventKind::PlayerActionFailed, &[], "SetAccessoryMult: not a number".to_string());
                    return;
                }
                let m = m.clamp(0.0, 5.0);
                self.levers.accessory_mult = Some(m);
                let text = format!("Lever: the Conspiracy sentence set to {m:.2} x Murder's");
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::PublicBounties(on) => {
                self.levers.public_bounties = *on;
                let text = if *on {
                    "Lever: the law posts a public bounty on every wanted suspect after one day"
                } else {
                    "Lever: public bounties back to the law's own delay"
                };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetLevy(on) => {
                self.levers.levy = *on;
                let text = if *on {
                    "Policy: the levy enacted (the wage schedule and the corps' upkeep levy)"
                } else {
                    "Policy: the levy repealed (wages from revenue, the tax band)"
                };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetSchoolMeals(on) => {
                self.levers.school_meals = *on;
                let text = if *on { "Policy: school meals enacted" } else { "Policy: school meals repealed" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetReserveRelease(on) => {
                self.levers.reserve_release = *on;
                let text =
                    if *on { "Policy: the Reserve release enacted" } else { "Policy: the Reserve release repealed" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetSafetyNet(on) => {
                let dole = if *on { self.config.levers.dole_per_day.min(10) } else { 0 };
                self.levers.dole_per_day = dole;
                self.levers.public_works = *on;
                self.levers.school_meals = *on;
                self.levers.reserve_release = *on;
                let text = if *on {
                    format!(
                        "Policy: the safety net enacted (a dole of {dole} a day, public works, school meals, the Reserve release)"
                    )
                } else {
                    "Policy: the safety net repealed".to_string()
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::SetStreetLegal(on) => {
                self.levers.vice.street_legal = *on;
                let text = if *on { "Policy: street work legal" } else { "Policy: street work swept" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetBrothelsLicensed(on) => {
                self.levers.vice.brothels_licensed = *on;
                let text = if *on { "Policy: brothels licensed" } else { "Policy: brothels unlicensed" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::SetOutsideWage(w) => {
                let w = if w.is_finite() { w.clamp(0.1, 1000.0) } else { 0.1 };
                self.econ.outside_wage_pin = Some(w);
                self.push_event(EventKind::PlayerAction, &[], format!("Outside wage set to {w:.2}"));
            }
            PlayerCommand::CloseWorld(closed) => {
                self.econ.world_closed = *closed;
                let text =
                    if *closed { "The World is closed: no exports, no imports" } else { "The World is open again" };
                self.push_event(EventKind::PlayerAction, &[], text.to_string());
            }
            PlayerCommand::OpenVenue { kind, district, owner } => {
                match crate::systems::jobs::open_venue(self, *kind, *district, *owner) {
                    Ok(b) => {
                        let text = format!("God opened {} ({})", self.name_of(b), kind.label());
                        self.push_event(EventKind::PlayerAction, &[b], text);
                    }
                    Err(e) => {
                        self.push_event(EventKind::PlayerActionFailed, &[], format!("OpenVenue: {e}"));
                    }
                }
            }
            PlayerCommand::HireAll { kind } => {
                let n = crate::systems::jobs::hire_all(self, *kind);
                self.push_event(EventKind::PlayerAction, &[], format!("God filled {n} {} vacancies", kind.label()));
            }
            PlayerCommand::FactionStrike { gang, district, days } => {
                if crate::systems::fviolence::faction_strike(self, *gang, *district, *days) {
                    let name = crate::systems::grudges::label(self, *gang);
                    let place = self.district_name(*district).to_string();
                    let text = format!("God sent {name} to strike {place} for {days} days");
                    self.push_event(EventKind::PlayerAction, &[*gang], text);
                } else {
                    self.push_event(EventKind::PlayerActionFailed, &[*gang], "FactionStrike: not a gang".to_string());
                }
            }
            PlayerCommand::PostContract { buyer, kind, target, price, brokered, deadline_days } => {
                match crate::systems::contracts::god_post(
                    self,
                    *buyer,
                    *kind,
                    *target,
                    *price,
                    *brokered,
                    *deadline_days,
                ) {
                    Ok(id) => {
                        let who = self.owner_label(*buyer);
                        let on = self.name_of(target.id());
                        let listed = self.contracts.get(&id).map_or(*price, |c| c.price);
                        let text = format!("God: {who} posted contract #{id} ({}) on {on} for {listed}", kind.label());
                        self.push_event(EventKind::PlayerAction, &[target.id()], text);
                    }
                    Err(e) => {
                        self.push_event(EventKind::PlayerActionFailed, &[], format!("PostContract: {e}"));
                    }
                }
            }
            PlayerCommand::Donate { mission, amount } => {
                match crate::systems::charity::god_donate(self, *mission, *amount) {
                    Ok(moved) => {
                        let text = format!("God gave {moved} to {}", self.name_of(*mission));
                        self.push_event(EventKind::PlayerAction, &[*mission], text);
                    }
                    Err(e) => {
                        self.push_event(EventKind::PlayerActionFailed, &[*mission], format!("Donate: {e}"));
                    }
                }
            }
            PlayerCommand::TakeContract { contract, taker } => {
                match crate::systems::contracts::god_take(self, *contract, *taker) {
                    Ok(()) => {
                        let text = format!("God: {} took contract #{contract}", self.owner_label(Some(*taker)));
                        self.push_event(EventKind::PlayerAction, &[*taker], text);
                    }
                    Err(e) => {
                        self.push_event(EventKind::PlayerActionFailed, &[*taker], format!("TakeContract: {e}"));
                    }
                }
            }
            PlayerCommand::SetRiotResponse(r) => {
                self.levers.riot_response = *r;
                let text = match r {
                    Some(r) => format!("Riot response pinned to {r:?}"),
                    None => "Riot response handed back to the captain".to_string(),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::Arrest(who) => match crate::systems::law::player_arrest(self, *who) {
                Ok(()) => {
                    let name = self.name_of(*who);
                    self.push_event(EventKind::PlayerAction, &[*who], format!("Arrested {name}"));
                }
                Err(e) => {
                    self.push_event(EventKind::PlayerActionFailed, &[*who], format!("Arrest: {e}"));
                }
            },
            PlayerCommand::Release(who) => match crate::systems::law::player_release(self, *who) {
                Ok(()) => {
                    let name = self.name_of(*who);
                    self.push_event(EventKind::PlayerAction, &[*who], format!("Released {name}"));
                }
                Err(e) => {
                    self.push_event(EventKind::PlayerActionFailed, &[*who], format!("Release: {e}"));
                }
            },
            PlayerCommand::KillAgent(_)
            | PlayerCommand::JailAgent { .. }
            | PlayerCommand::FreeAgent(_)
            | PlayerCommand::FundGang { .. }
            | PlayerCommand::SeizeGangTreasury(_)
            | PlayerCommand::KillGang(_)
            | PlayerCommand::JailGang { .. }
            | PlayerCommand::FireAllGuards
            | PlayerCommand::SetTreasury(_)
            | PlayerCommand::FundCorp { .. }
            | PlayerCommand::BankruptCorp(_)
            | PlayerCommand::SeizeBuildings { .. }
            | PlayerCommand::KillExec(_)
            | PlayerCommand::KillStaff(_)
            | PlayerCommand::SetCorpOrder { .. }
            | PlayerCommand::StrikeNow(_)
            | PlayerCommand::WipeTreasuries
            | PlayerCommand::Riot(_)
            | PlayerCommand::Litter { .. }
            | PlayerCommand::SplitGang(_)
            | PlayerCommand::Derelict(_)
            | PlayerCommand::BuyBuilding { .. }
            | PlayerCommand::GrantAsset { .. }
            | PlayerCommand::FloodGuns { .. }
            | PlayerCommand::Wreck(_)
            | PlayerCommand::ChromeEveryone { .. }
            | PlayerCommand::FloodStims { .. }
            | PlayerCommand::Brick(_)
            | PlayerCommand::Chase(_)
            | PlayerCommand::WipeData(_)
            | PlayerCommand::SetTech { .. }
            | PlayerCommand::GrantData { .. }
            | PlayerCommand::GrantDeck { .. }
            | PlayerCommand::SetIce { .. }
            | PlayerCommand::Fry(_)
            | PlayerCommand::RunNow { .. }
            | PlayerCommand::WipeCorpData(_)
            | PlayerCommand::GrantDecks { .. }
            | PlayerCommand::SetCorpIce { .. }
            | PlayerCommand::PlantRumour { .. }
            | PlayerCommand::SetReputation { .. }
            | PlayerCommand::GrantSkill { .. }
            | PlayerCommand::SetCreed { .. }
            | PlayerCommand::DeclareVendetta { .. }
            | PlayerCommand::KillFriend { .. }
            | PlayerCommand::Hunt { .. } => {
                let _ = match self.cmd_god(cmd) {
                    Ok((actors, text)) => self.push_event(EventKind::PlayerAction, &actors, format!("God: {text}")),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("God: {e}")),
                };
            }
            PlayerCommand::SetCityRent(rent) => {
                self.levers.city_rent = rent.map(|r| r.clamp(0, 20));
                let [a, b, c] = self.levers.city_rent;
                self.push_event(
                    EventKind::PlayerAction,
                    &[],
                    format!("City rent set to {a}/{b}/{c} per Block per day"),
                );
            }
            PlayerCommand::SetRentCap(cap) => {
                self.levers.rent_cap = cap.map(|c| c.max(0));
                let text = match self.levers.rent_cap {
                    Some(c) => format!("Rent capped at {c} per Block per day"),
                    None => "Rent cap lifted".to_string(),
                };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::NoCityEvictions(on) => {
                self.levers.no_city_evictions = *on;
                let text = if *on { "The city stops evicting" } else { "The city evicts again" };
                self.push_event(EventKind::PlayerAction, &[], text);
            }
            PlayerCommand::Nationalise(b) => {
                let _ = match crate::systems::ownership::nationalise(self, *b) {
                    Ok(text) => self.push_event(EventKind::PlayerAction, &[*b], text),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*b], format!("Nationalise: {e}")),
                };
            }
            PlayerCommand::Subsidise { corp, amount } => {
                let _ = match crate::systems::ownership::subsidise(self, *corp, *amount) {
                    Ok(text) => self.push_event(EventKind::PlayerAction, &[*corp], text),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*corp], format!("Subsidise: {e}")),
                };
            }
            PlayerCommand::BreakUp(corp) => {
                let _ = match crate::systems::corps::break_up(self, *corp) {
                    Ok(s) => {
                        let text =
                            format!("Broke up {} into {}", self.owner_label(Some(*corp)), self.owner_label(Some(s)));
                        self.push_event(EventKind::PlayerAction, &[*corp, s], text)
                    }
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*corp], format!("BreakUp: {e}")),
                };
            }
            PlayerCommand::DemolishHome(home) => {
                let _ = match self.cmd_demolish_home(*home) {
                    Ok(n) => self.push_event(
                        EventKind::PlayerAction,
                        &[*home],
                        format!("Demolished Block#{} ({n} residents made homeless)", home.index),
                    ),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[*home], format!("DemolishHome: {e}")),
                };
            }
            PlayerCommand::BuildHome { rect } => {
                let _ = match self.cmd_build_home(*rect) {
                    Ok((id, housed)) => self.push_event(
                        EventKind::PlayerAction,
                        &[id],
                        format!("Built Block#{} at ({}, {}); {housed} moved in", id.index, rect.x, rect.y),
                    ),
                    Err(e) => self.push_event(EventKind::PlayerActionFailed, &[], format!("BuildHome: {e}")),
                };
            }
        }
    }
}
