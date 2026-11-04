use super::*;

/// `day=90:release_reserve=1500` → `(tick, lever)`. God levers name a gang
/// by index: `kill_leader=0`, `jail_gang=0:60`, `kill_gang=0`,
/// `fund_gang=1:10000`, `seize_gang=0`, `fire_guards=1`, `treasury=-50000`;
/// M11 names a corp by its seeding slot: `breakup=0`, and the corp god levers
/// `fund_corp=0:200000`, `bankrupt_corp=0`, `seize_corp=3:gang0` (or
/// `:city`, `:<slot>`), `kill_exec=0`, `kill_staff=0`,
/// `corp_order=3:Squeeze:30` (or `1:Squeeze:Housing:30`), `strike=0`,
/// `wipe_corps=1`. M12 (plan D42): `guard_weight=<district>:<weight>`,
/// `stance=<district>:sweep|patrol|cordon|withdrawn|crackdown<gang index>|auto`,
/// `sanitation=12`, `sanitation_weight=<district>:<weight>`,
/// `curfew=<district>:on|off`, `riot_response=crush|disperse|contain|auto`;
/// M12 god levers `riot=<district>`, `litter=<district>:<level 0..1>`,
/// `split_gang=<gang index>`, `derelict=<building index>`,
/// `buy_building=<city|gang<i>|corp<slot>|agent index>:<building index>:<price>`.
/// M13 (plan D48): `stims_legal=on|off`, `asset_tax=car:0.5` (a class:
/// `moto|car|truck|flyer|implant|robot|pack|bridge`), `impound=on|off`;
/// M13 god levers `grant_asset=<agent index>:<kind>:<tier>` (kinds
/// `moto|car|truck|flyer|arms|legs|nerves|eyes|skin|robot|pack|bridge`),
/// `wreck=<asset index>`, `chrome_everyone=<tier>`,
/// `flood_stims=<district>:<n>`, `brick=<corp slot>|agent:<index>`,
/// `chase=<agent index>`. M14 god levers (plan V42, phase 1):
/// `wipe_data=<building index>`, `set_tech=<corp slot>:<chrome|deck|industry>:<tier>`,
/// `grant_data=<corp slot>|gang<i>:<chrome|deck|industry>:<units>`. M14
/// levers (phase 4): `city_ice=<0..3>` (the Treasury's and Precinct's ICE),
/// `data_tax=<0..1>` (an extra share of Data sales to the Treasury),
/// `hack_sentence=intrusion|data_theft:<days>`; god `grant_deck=<agent
/// index>:<tier>`, `set_ice=<building index>:<tier>`, `fry=<agent index>`,
/// `run_now=<agent index>:<building index|corp<slot>>:<data|wipe|ledger|door>`;
/// phase 5's plurals `wipe_corp_data=<corp slot>`,
/// `grant_decks=<district>:<n>:<tier>`, `set_corp_ice=<corp slot>:<tier>`.
/// M15 god levers (plan W42, phase 1):
/// `plant_rumour=<agent>:<deed>:<object agent|none>:<district>:<reach>`,
/// `set_reputation=<agent>:<dread|standing|honour|heat>:<value>:<days>`;
/// phase 2: `grant_skill=<agent>:<skill>:<v>`,
/// `grant_skill=dregs<n>:<skill>:<v>:suit`, `set_creed=<gang i>:purist|none`;
/// phase 3: `declare_vendetta=<faction>:<faction>:<w>` (factions `gang<i>`,
/// `corp<slot>`, `law`), `kill_friend=<agent|leader<g>>:<agent|member<g>>`,
/// `hunt=<agent>:<agent>`. M15 levers (phase 4): `press_licence=on|off`,
/// `news_tax=<0..1>`, `censor=gang<i>|corp<slot>|law[:off]`.
pub(super) fn parse_lever(spec: &str) -> Result<(u64, Lever), String> {
    let (day_part, cmd_part) =
        spec.split_once(':').ok_or_else(|| format!("{spec}: expected day=<D>:<lever>=<value>"))?;
    let day: u64 = day_part
        .strip_prefix("day=")
        .ok_or_else(|| format!("{spec}: expected day=<D>"))?
        .parse()
        .map_err(|e| format!("{spec}: bad day: {e}"))?;
    let (name, value) = cmd_part.split_once('=').ok_or_else(|| format!("{spec}: expected <lever>=<value>"))?;
    let num = |what: &str| value.parse::<f64>().map_err(|e| format!("{spec}: bad {what}: {e}"));
    let idx = |v: &str| v.parse::<usize>().map_err(|e| format!("{spec}: bad gang index: {e}"));
    let slot = |v: &str| v.parse::<u8>().map_err(|e| format!("{spec}: bad corp slot: {e}"));
    // `<gang index>:<n>`
    let pair = || -> Result<(usize, i64), String> {
        let (g, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <gang index>:<n>"))?;
        Ok((idx(g)?, n.parse::<i64>().map_err(|e| format!("{spec}: bad number: {e}"))?))
    };
    let god = match name {
        "kill_leader" => Some(Lever::KillLeader(idx(value)?)),
        "kill_gang" => Some(Lever::KillGang(idx(value)?)),
        "seize_gang" => Some(Lever::SeizeGang(idx(value)?)),
        "jail_gang" => {
            let (g, d) = pair()?;
            Some(Lever::JailGang(g, u32::try_from(d).map_err(|e| format!("{spec}: bad days: {e}"))?))
        }
        "fund_gang" => {
            let (g, n) = pair()?;
            Some(Lever::FundGang(g, n))
        }
        "breakup" => Some(Lever::BreakUp(slot(value)?)),
        "fund_corp" => {
            let (s, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <corp slot>:<coins>"))?;
            Some(Lever::FundCorp(slot(s)?, n.parse::<i64>().map_err(|e| format!("{spec}: bad coins: {e}"))?))
        }
        "bankrupt_corp" => Some(Lever::BankruptCorp(slot(value)?)),
        "seize_corp" => {
            let (s, to) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <corp slot>:<to>"))?;
            let to = match to {
                "city" => SeizeTo::City,
                t if t.starts_with("gang") => SeizeTo::Gang(idx(&t[4..])?),
                t => SeizeTo::Corp(slot(t)?),
            };
            Some(Lever::SeizeCorp(slot(s)?, to))
        }
        "stance" => {
            let (d, st) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<stance>"))?;
            let d = d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?;
            let st = st.to_ascii_lowercase();
            match st.strip_prefix("crackdown") {
                Some(g) => Some(Lever::StanceCrackdown(d, idx(g)?)),
                None => {
                    let stance = match st.as_str() {
                        "auto" => None,
                        "patrol" => Some(citysim::Stance::Patrol),
                        "sweep" => Some(citysim::Stance::Sweep),
                        "cordon" => Some(citysim::Stance::Cordon),
                        "withdrawn" => Some(citysim::Stance::Withdrawn),
                        _ => {
                            return Err(format!(
                                "{spec}: stance must be auto|patrol|sweep|cordon|withdrawn|crackdown<gang index>"
                            ))
                        }
                    };
                    Some(Lever::Cmd(PlayerCommand::SetStance { district: citysim::DistrictId(d), stance }))
                }
            }
        }
        "split_gang" => Some(Lever::SplitGang(idx(value)?)),
        "derelict" => Some(Lever::Derelict(value.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?)),
        "buy_building" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [who, b, price] = parts[..] else {
                return Err(format!("{spec}: expected <buyer>:<building index>:<price>"));
            };
            let buyer = match who {
                "city" => Buyer::City,
                w if w.starts_with("gang") => Buyer::Gang(idx(&w[4..])?),
                w if w.starts_with("corp") => Buyer::Corp(slot(&w[4..])?),
                w => Buyer::Agent(w.parse::<u32>().map_err(|e| format!("{spec}: bad buyer: {e}"))?),
            };
            Some(Lever::BuyBuilding(
                buyer,
                b.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?,
                price.parse::<i64>().map_err(|e| format!("{spec}: bad price: {e}"))?,
            ))
        }
        "grant_asset" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [a, k, t] = parts[..] else {
                return Err(format!("{spec}: expected <agent index>:<kind>:<tier>"));
            };
            Some(Lever::GrantAsset(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                parse_asset_kind(k).ok_or_else(|| format!("{spec}: unknown asset kind {k}"))?,
                t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "wreck" => Some(Lever::Wreck(value.parse::<u32>().map_err(|e| format!("{spec}: bad asset: {e}"))?)),
        "brick" => Some(Lever::Brick(match value.strip_prefix("agent:") {
            Some(a) => Lender::Agent(a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?),
            None => Lender::Corp(slot(value)?),
        })),
        "chase" => Some(Lever::Chase(value.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?)),
        "wipe_data" => Some(Lever::WipeData(value.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?)),
        "set_tech" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [s, t, n] = parts[..] else {
                return Err(format!("{spec}: expected <corp slot>:<track>:<tier>"));
            };
            Some(Lever::SetTech(
                slot(s)?,
                citysim::virt::Track::parse(t).ok_or_else(|| format!("{spec}: unknown track {t}"))?,
                n.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "grant_data" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [f, t, n] = parts[..] else {
                return Err(format!("{spec}: expected <corp slot>|gang<i>:<track>:<units>"));
            };
            let faction = match f.strip_prefix("gang") {
                Some(g) => Faction::Gang(idx(g)?),
                None => Faction::Corp(slot(f)?),
            };
            Some(Lever::GrantData(
                faction,
                citysim::virt::Track::parse(t).ok_or_else(|| format!("{spec}: unknown track {t}"))?,
                n.parse::<u32>().map_err(|e| format!("{spec}: bad units: {e}"))?,
            ))
        }
        "grant_deck" => {
            let (a, t) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <agent index>:<tier>"))?;
            Some(Lever::GrantDeck(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "set_ice" => {
            let (b, t) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <building index>:<tier>"))?;
            Some(Lever::SetIce(
                b.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?,
                t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "fry" => Some(Lever::Fry(value.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?)),
        "wipe_corp_data" => Some(Lever::WipeCorpData(slot(value)?)),
        "grant_decks" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [d, n, t] = parts[..] else {
                return Err(format!("{spec}: expected <district>:<n>:<tier>"));
            };
            Some(Lever::GrantDecks(
                d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?,
                n.parse::<u16>().map_err(|e| format!("{spec}: bad count: {e}"))?,
                t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            ))
        }
        "set_corp_ice" => {
            let (s, t) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <corp slot>:<tier>"))?;
            Some(Lever::SetCorpIce(slot(s)?, t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?))
        }
        "run_now" => {
            use citysim::virt::Purpose;
            let parts: Vec<&str> = value.split(':').collect();
            let [a, t, p] = parts[..] else {
                return Err(format!("{spec}: expected <agent index>:<building index|corp<slot>>:<purpose>"));
            };
            let target = match t.strip_prefix("corp") {
                Some(s) => RunTarget::Corp(slot(s)?),
                None => RunTarget::Building(t.parse::<u32>().map_err(|e| format!("{spec}: bad target: {e}"))?),
            };
            let purpose = match p.to_ascii_lowercase().as_str() {
                "data" => Purpose::Data { wipe: false },
                "wipe" => Purpose::Data { wipe: true },
                "ledger" => Purpose::Ledger,
                "door" => Purpose::Door,
                _ => return Err(format!("{spec}: purpose must be data|wipe|ledger|door")),
            };
            Some(Lever::RunNow(a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?, target, purpose))
        }
        "plant_rumour" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [a, d, o, di, r] = parts[..] else {
                return Err(format!("{spec}: expected <agent>:<deed>:<object agent|none>:<district>:<reach>"));
            };
            let deed = citysim::word::Deed::parse(d).ok_or_else(|| format!("{spec}: unknown deed {d}"))?;
            let object = if o.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(o.parse::<u32>().map_err(|e| format!("{spec}: bad object: {e}"))?)
            };
            Some(Lever::PlantRumour(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                deed,
                object,
                di.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?,
                r.parse::<f32>().map_err(|e| format!("{spec}: bad reach: {e}"))?,
            ))
        }
        "set_reputation" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [a, x, v, d] = parts[..] else {
                return Err(format!("{spec}: expected <agent>:<dread|standing|honour|heat>:<v>:<days>"));
            };
            let axis = citysim::word::Axis::parse(x).ok_or_else(|| format!("{spec}: unknown axis {x}"))?;
            Some(Lever::SetReputation(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                axis,
                v.parse::<f32>().map_err(|e| format!("{spec}: bad value: {e}"))?,
                d.parse::<u16>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            ))
        }
        "grant_skill" => {
            let parts: Vec<&str> = value.split(':').collect();
            let (who, sk, v, suit) = match parts[..] {
                [w, s, v] => (w, s, v, false),
                [w, s, v, "suit"] => (w, s, v, true),
                _ => return Err(format!("{spec}: expected <agent|dregs<n>>:<skill>:<v>[:suit]")),
            };
            let skill = citysim::word::SocialSkill::parse(sk).ok_or_else(|| format!("{spec}: unknown skill {sk}"))?;
            let target = match who.strip_prefix("dregs") {
                Some(n) => SkillTarget::Dregs(n.parse::<u32>().map_err(|e| format!("{spec}: bad dregs count: {e}"))?),
                None => SkillTarget::Agent(who.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?),
            };
            if suit && matches!(target, SkillTarget::Agent(_)) {
                return Err(format!("{spec}: :suit goes with dregs<n>"));
            }
            Some(Lever::GrantSkill(
                target,
                skill,
                v.parse::<f32>().map_err(|e| format!("{spec}: bad value: {e}"))?,
                suit,
            ))
        }
        "set_creed" => {
            let (g, c) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <gang>:purist|none"))?;
            let creed = match c.to_ascii_lowercase().as_str() {
                "purist" => Some(citysim::word::Creed::Purist),
                "none" => None,
                _ => return Err(format!("{spec}: unknown creed {c}")),
            };
            Some(Lever::SetCreed(g.parse::<usize>().map_err(|e| format!("{spec}: bad gang: {e}"))?, creed))
        }
        "declare_vendetta" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [a, b, w] = parts[..] else {
                return Err(format!("{spec}: expected <faction>:<faction>:<weight>"));
            };
            Some(Lever::DeclareVendetta(
                parse_faction(a).map_err(|e| format!("{spec}: {e}"))?,
                parse_faction(b).map_err(|e| format!("{spec}: {e}"))?,
                w.parse::<f32>().map_err(|e| format!("{spec}: bad weight: {e}"))?,
            ))
        }
        "kill_friend" => {
            let (o, b) =
                value.split_once(':').ok_or_else(|| format!("{spec}: expected <agent|leader<g>>:<agent|member<g>>"))?;
            Some(Lever::KillFriend(
                parse_who(o).map_err(|e| format!("{spec}: {e}"))?,
                parse_who(b).map_err(|e| format!("{spec}: {e}"))?,
            ))
        }
        "hunt" => {
            let (h, t) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <agent>:<agent>"))?;
            Some(Lever::Hunt(
                h.parse::<u32>().map_err(|e| format!("{spec}: bad hunter: {e}"))?,
                t.parse::<u32>().map_err(|e| format!("{spec}: bad target: {e}"))?,
            ))
        }
        "censor" => {
            let (f, on) = match value.split_once(':') {
                Some((f, flag)) => (f, on_off(flag).ok_or_else(|| format!("{spec}: censor flag must be on|off"))?),
                None => (value, true),
            };
            Some(Lever::Censor(parse_faction(f).map_err(|e| format!("{spec}: {e}"))?, on))
        }
        // L2 (L37): `open_venue=<kind>:<district>:<city|corp<slot>|gang<i>>`.
        "open_venue" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [k, d, o] = parts[..] else {
                return Err(format!("{spec}: expected <kind>:<district>:<city|corp<slot>|gang<i>>"));
            };
            let kind = citysim::BuildingKind::parse(k).ok_or_else(|| format!("{spec}: unknown kind {k}"))?;
            let owner = if o.eq_ignore_ascii_case("city") {
                None
            } else {
                Some(parse_faction(o).map_err(|e| format!("{spec}: {e}"))?)
            };
            Some(Lever::OpenVenue(kind, d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?, owner))
        }
        // L2 (L37, phase 4): `faction_strike=<gang i>:<district>:<days>`.
        "faction_strike" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [g, d, n] = parts[..] else {
                return Err(format!("{spec}: expected <gang i>:<district>:<days>"));
            };
            Some(Lever::FactionStrike(
                g.parse::<usize>().map_err(|e| format!("{spec}: bad gang: {e}"))?,
                d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?,
                n.parse::<u64>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            ))
        }
        // M16a (plan C39).
        "post_contract" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [who, kind, target, price, brokered, days] = parts[..] else {
                return Err(format!(
                    "{spec}: expected <buyer>:<hit|beat|guard|locate>:<agent|b<building>>:<price>:<0|1>:<days>"
                ));
            };
            let buyer = match who {
                "city" => Buyer::City,
                w if w.starts_with("gang") => Buyer::Gang(idx(&w[4..])?),
                w if w.starts_with("corp") => Buyer::Corp(slot(&w[4..])?),
                w => Buyer::Agent(w.parse::<u32>().map_err(|e| format!("{spec}: bad buyer: {e}"))?),
            };
            let kind = citysim::contract::ContractKind::parse(kind)
                .ok_or_else(|| format!("{spec}: kind must be hit|beat|guard|locate"))?;
            let target = match target.strip_prefix('b') {
                Some(b) => {
                    ContractTarget::Building(b.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?)
                }
                None => ContractTarget::Agent(target.parse::<u32>().map_err(|e| format!("{spec}: bad target: {e}"))?),
            };
            Some(Lever::PostContract(
                buyer,
                kind,
                target,
                price.parse::<i64>().map_err(|e| format!("{spec}: bad price: {e}"))?,
                on_off(brokered).ok_or_else(|| format!("{spec}: brokered must be 0|1"))?,
                days.parse::<u16>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            ))
        }
        // M16b phase 4 (spec § 8).
        "prison_permit" => Some(Lever::Cmd(PlayerCommand::SetPrisonPermit(
            on_off(value).ok_or_else(|| format!("{spec}: expected on|off"))?,
        ))),
        "expose_secret" => Some(Lever::Cmd(PlayerCommand::ExposeSecret(
            value.parse::<u64>().map_err(|e| format!("{spec}: bad coercion: {e}"))?,
        ))),
        "call_vote" => Some(Lever::M16b(M16bLever::CallVote(slot(value)?))),
        "wound" => {
            Some(Lever::M16b(M16bLever::Wound(value.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?)))
        }
        "subscribe" => {
            let (a, c) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <agent>:<corp slot>"))?;
            Some(Lever::M16b(M16bLever::Subscribe(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                slot(c)?,
            )))
        }
        "tag_agent" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [a, f, t] = parts[..] else { return Err(format!("{spec}: expected <agent>:<faction>:<tier>")) };
            Some(Lever::M16b(M16bLever::TagAgent(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                parse_faction(f).map_err(|e| format!("{spec}: {e}"))?,
                t.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?,
            )))
        }
        "hostage" => {
            let parts: Vec<&str> = value.split(':').collect();
            let (a, h, b, d) = match parts[..] {
                [a, h, b] => (a, h, b, "none"),
                [a, h, b, d] => (a, h, b, d),
                _ => return Err(format!("{spec}: expected <agent>:<holder|none>:<building>[:<demand>]")),
            };
            Some(Lever::M16b(M16bLever::Hostage {
                captive: a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                holder: if h == "none" { None } else { Some(parse_faction(h).map_err(|e| format!("{spec}: {e}"))?) },
                at: b.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?,
                demand: parse_demand(d).map_err(|e| format!("{spec}: {e}"))?,
            }))
        }
        "coerce" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [a, o, l, d] = parts[..] else {
                return Err(format!("{spec}: expected <agent>:<over>:<threat|debt<n>>:<demand>"));
            };
            let lev = match l.strip_prefix("debt") {
                Some(n) => LevSpec::Debt(n.parse::<i64>().map_err(|e| format!("{spec}: bad debt: {e}"))?),
                None if l == "threat" => LevSpec::Threat,
                None => return Err(format!("{spec}: leverage must be threat|debt<n>")),
            };
            let demand = parse_demand(d).map_err(|e| format!("{spec}: {e}"))?;
            if demand == DemandSpec::None {
                return Err(format!("{spec}: coerce needs a demand"));
            }
            Some(Lever::M16b(M16bLever::Coerce(
                a.parse::<u32>().map_err(|e| format!("{spec}: bad agent: {e}"))?,
                o.parse::<u32>().map_err(|e| format!("{spec}: bad over: {e}"))?,
                lev,
                demand,
            )))
        }
        "take_contract" => {
            let (c, t) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <id>:<agent>"))?;
            Some(Lever::TakeContract(
                c.parse::<u64>().map_err(|e| format!("{spec}: bad contract: {e}"))?,
                t.parse::<u32>().map_err(|e| format!("{spec}: bad taker: {e}"))?,
            ))
        }
        // Real economy (plan E47).
        "donate" => {
            let (b, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <mission>:<coins>"))?;
            Some(Lever::Donate(
                b.parse::<u32>().map_err(|e| format!("{spec}: bad building: {e}"))?,
                n.parse::<i64>().map_err(|e| format!("{spec}: bad coins: {e}"))?,
            ))
        }
        "kill_exec" => Some(Lever::KillExec(slot(value)?)),
        "kill_staff" => Some(Lever::KillStaff(slot(value)?)),
        "strike" => Some(Lever::Strike(slot(value)?)),
        // `<slot>:<Order>:<days>` or `<slot>:<Order>:<Niche>:<days>`.
        "corp_order" => {
            let parts: Vec<&str> = value.split(':').collect();
            let (s, o, n, d) = match parts[..] {
                [s, o, d] => (s, o, None, d),
                [s, o, n, d] => (s, o, Some(n), d),
                _ => return Err(format!("{spec}: expected <corp slot>:<Order>[:<Niche>]:<days>")),
            };
            let order = citysim::CorpOrder::ALL
                .into_iter()
                .find(|x| x.to_string().eq_ignore_ascii_case(o))
                .ok_or_else(|| format!("{spec}: unknown corp order {o}"))?;
            let niche = match n {
                Some(n) => Some(
                    citysim::Niche::ALL
                        .into_iter()
                        .find(|x| x.label().eq_ignore_ascii_case(n))
                        .ok_or_else(|| format!("{spec}: unknown niche {n}"))?,
                ),
                None => None,
            };
            Some(Lever::CorpOrder(
                slot(s)?,
                order,
                niche,
                d.parse::<u32>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            ))
        }
        _ => None,
    };
    if let Some(l) = god {
        return Ok((day * TICKS_PER_DAY, l));
    }
    let cmd = match name {
        "release_reserve" => PlayerCommand::ReleaseReserve { amount: num("amount")? as u32 },
        "tax_rate" => PlayerCommand::SetTaxRate(num("rate")? as f32),
        // Real economy E47 (phase 3a): `tax=auto` releases a pin to the band;
        // `tax=<rate>` is `tax_rate=<rate>`.
        "tax" if value.eq_ignore_ascii_case("auto") => PlayerCommand::SetTaxAuto,
        "tax" => PlayerCommand::SetTaxRate(num("rate")? as f32),
        "sentence_mult" => PlayerCommand::SetSentenceMult(num("multiplier")? as f32),
        "guard_count" => PlayerCommand::SetGuardCount(num("count")? as u16),
        // Jobs v2 J17 (P5a): `civic=auto` releases a guard/Sanitation pin to the budget.
        "civic" if value.eq_ignore_ascii_case("auto") => PlayerCommand::SetCivicAuto,
        "immigration_per_week" => PlayerCommand::SetImmigrationPerWeek(num("count")? as u16),
        "dole_per_day" => PlayerCommand::SetDolePerDay(num("coins")? as u8),
        "law_posture" => PlayerCommand::SetLawPosture(match value.to_ascii_lowercase().as_str() {
            "auto" => None,
            "patrol" => Some(Posture::Patrol),
            "crackdown" => Some(Posture::Crackdown),
            "garrison" => Some(Posture::Garrison),
            _ => return Err(format!("{spec}: law_posture must be Auto|Patrol|Crackdown|Garrison")),
        }),
        "fire_guards" => PlayerCommand::FireAllGuards,
        "guard_weight" => {
            let (d, wt) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<weight>"))?;
            PlayerCommand::SetGuardWeight {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                weight: wt.parse::<f32>().map_err(|e| format!("{spec}: bad weight: {e}"))?,
            }
        }
        "sanitation" => PlayerCommand::SetSanitation(num("count")?.clamp(0.0, 65535.0) as u16),
        "sanitation_weight" => {
            let (d, wt) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<weight>"))?;
            PlayerCommand::SetSanitationWeight {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                weight: wt.parse::<f32>().map_err(|e| format!("{spec}: bad weight: {e}"))?,
            }
        }
        "curfew" => {
            let (d, on) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:on|off"))?;
            let on = match on.to_ascii_lowercase().as_str() {
                "on" | "1" | "true" => true,
                "off" | "0" | "false" => false,
                _ => return Err(format!("{spec}: curfew must be on|off")),
            };
            PlayerCommand::SetCurfew {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                on,
            }
        }
        // M13 D48: `stims_legal=on|off`, `asset_tax=car:0.5`, `impound=on|off`,
        // god `chrome_everyone=<tier>`, `flood_stims=<district>:<n>`.
        "stims_legal" => {
            PlayerCommand::SetStimsLegal(on_off(value).ok_or_else(|| format!("{spec}: stims_legal must be on|off"))?)
        }
        "asset_tax" => {
            let (k, r) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <class>:<rate>"))?;
            PlayerCommand::SetAssetTax {
                kind: parse_asset_class(k).ok_or_else(|| format!("{spec}: unknown asset class {k}"))?,
                rate: r.parse::<f32>().map_err(|e| format!("{spec}: bad rate: {e}"))?,
            }
        }
        "impound" => PlayerCommand::SetImpound(on_off(value).ok_or_else(|| format!("{spec}: impound must be on|off"))?),
        "chrome_everyone" => {
            PlayerCommand::ChromeEveryone { tier: value.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))? }
        }
        "flood_stims" => {
            let (d, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<n>"))?;
            PlayerCommand::FloodStims {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                n: n.parse::<u32>().map_err(|e| format!("{spec}: bad doses: {e}"))?,
            }
        }
        "riot_response" => PlayerCommand::SetRiotResponse(match value.to_ascii_lowercase().as_str() {
            "auto" => None,
            "contain" => Some(citysim::RiotResponse::Contain),
            "disperse" => Some(citysim::RiotResponse::Disperse),
            "crush" => Some(citysim::RiotResponse::Crush),
            _ => return Err(format!("{spec}: riot_response must be auto|contain|disperse|crush")),
        }),
        "riot" => PlayerCommand::Riot(citysim::DistrictId(
            value.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?,
        )),
        "litter" => {
            let (d, l) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<level>"))?;
            PlayerCommand::Litter {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                level: l.parse::<f32>().map_err(|e| format!("{spec}: bad level: {e}"))?,
            }
        }
        "treasury" => PlayerCommand::SetTreasury(value.parse::<i64>().map_err(|e| format!("{spec}: bad coins: {e}"))?),
        // M11: `city_rent=0/1/2` (Sump/Mid/Spire), `rent_cap=3` or `rent_cap=none`, `no_city_evictions=1`.
        "city_rent" => {
            let parts: Vec<i64> = value
                .split('/')
                .map(|v| v.parse::<i64>().map_err(|e| format!("{spec}: bad rent: {e}")))
                .collect::<Result<_, _>>()?;
            let rent: [i64; 3] =
                parts.try_into().map_err(|_| format!("{spec}: city_rent takes three values, Sump/Mid/Spire"))?;
            PlayerCommand::SetCityRent(rent)
        }
        "rent_cap" => PlayerCommand::SetRentCap(match value {
            "none" => None,
            v => Some(v.parse::<i64>().map_err(|e| format!("{spec}: bad cap: {e}"))?),
        }),
        "no_city_evictions" => PlayerCommand::NoCityEvictions(num("flag")? != 0.0),
        "wipe_corps" => PlayerCommand::WipeTreasuries,
        // M14 V42: `city_ice=<0..3>`, `data_tax=<0..1>`, `hack_sentence=intrusion|data_theft:<days>`.
        "city_ice" => PlayerCommand::SetCityIce(value.parse::<u8>().map_err(|e| format!("{spec}: bad tier: {e}"))?),
        "data_tax" => PlayerCommand::SetDataTax(num("rate")? as f32),
        // M15 W42: `press_licence=on|off`, `news_tax=<0..1>`.
        "press_licence" => PlayerCommand::SetPressLicence(
            on_off(value).ok_or_else(|| format!("{spec}: press_licence must be on|off"))?,
        ),
        "news_tax" => PlayerCommand::SetNewsTax(num("rate")? as f32),
        // L2 (L37): `public_works=on|off`, `export=on|off`,
        // `export_price=<food|parts|data>:<n>`, `hire_all=<kind>`.
        // L2 phase 2 (L37): `leisure_tax=<f>`, `ban_gambling=on|off`,
        // `close_leisure=<district>:<days>`, `set_fun=<district>:<v>`.
        "leisure_tax" => PlayerCommand::SetLeisureTax(num("rate")? as f32),
        "ban_gambling" => {
            PlayerCommand::BanGambling(on_off(value).ok_or_else(|| format!("{spec}: ban_gambling must be on|off"))?)
        }
        "close_leisure" => {
            let (d, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<days>"))?;
            PlayerCommand::CloseLeisure {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                days: n.parse::<u32>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            }
        }
        "set_fun" => {
            let (d, v) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <district>:<v>"))?;
            PlayerCommand::SetFun {
                district: citysim::DistrictId(d.parse::<u8>().map_err(|e| format!("{spec}: bad district: {e}"))?),
                value: v.parse::<f32>().map_err(|e| format!("{spec}: bad value: {e}"))?,
            }
        }
        "public_works" => {
            PlayerCommand::SetPublicWorks(on_off(value).ok_or_else(|| format!("{spec}: public_works must be on|off"))?)
        }
        "export" => PlayerCommand::SetExport(on_off(value).ok_or_else(|| format!("{spec}: export must be on|off"))?),
        // P10 (docs/P10_POLICY.md): the policy levers (`dole_per_day=<n>`
        // and `public_works=on|off` above are two of them).
        "levy" => PlayerCommand::SetLevy(on_off(value).ok_or_else(|| format!("{spec}: levy must be on|off"))?),
        "school_meals" => {
            PlayerCommand::SetSchoolMeals(on_off(value).ok_or_else(|| format!("{spec}: school_meals must be on|off"))?)
        }
        "reserve_release" => PlayerCommand::SetReserveRelease(
            on_off(value).ok_or_else(|| format!("{spec}: reserve_release must be on|off"))?,
        ),
        "safety_net" => {
            PlayerCommand::SetSafetyNet(on_off(value).ok_or_else(|| format!("{spec}: safety_net must be on|off"))?)
        }
        "export_price" => {
            let (g, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <food|parts|data>:<n>"))?;
            PlayerCommand::SetExportPrice {
                good: citysim::outside::ExportGood::parse(g).ok_or_else(|| format!("{spec}: unknown good {g}"))?,
                price: n.parse::<i64>().map_err(|e| format!("{spec}: bad price: {e}"))?,
            }
        }
        "hire_all" => PlayerCommand::HireAll {
            kind: citysim::BuildingKind::parse(value).ok_or_else(|| format!("{spec}: unknown kind {value}"))?,
        },
        // Real economy phase 1 (plan E47): `appetite=<good>:<mult|auto>`,
        // `export_cap=<good>:<n|auto>`, `ask=<good>:<mult>`, `customs=<rate>`,
        // `close_world=on|off`.
        "appetite" => {
            let (g, m) =
                value.split_once(':').ok_or_else(|| format!("{spec}: expected <food|parts|data>:<mult|auto>"))?;
            PlayerCommand::SetAppetite {
                good: citysim::outside::ExportGood::parse(g).ok_or_else(|| format!("{spec}: unknown good {g}"))?,
                mult: if m.eq_ignore_ascii_case("auto") {
                    None
                } else {
                    Some(m.parse::<f32>().map_err(|e| format!("{spec}: bad mult: {e}"))?)
                },
            }
        }
        // M17 phase 1: `ambient_appetite=<mult|auto>`, the Ambient book's appetite pin.
        "ambient_appetite" => PlayerCommand::SetAmbientAppetite(if value.eq_ignore_ascii_case("auto") {
            None
        } else {
            Some(value.parse::<f32>().map_err(|e| format!("{spec}: bad mult: {e}"))?)
        }),
        // M17 phase 2: `embargo=<account id>:on|off`, `tariff=<rate>`.
        "embargo" => {
            let (id, v) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <account id>:on|off"))?;
            PlayerCommand::Embargo {
                id: id.parse::<u32>().map_err(|e| format!("{spec}: bad account id: {e}"))?,
                on: on_off(v).ok_or_else(|| format!("{spec}: embargo must be on|off"))?,
            }
        }
        "tariff" => PlayerCommand::SetTariff(value.parse::<f32>().map_err(|e| format!("{spec}: bad tariff: {e}"))?),
        // M17 phase 3: `force_reinforce=<account id>:<bodies>`, `outside_order=<account id>:<order>:<days>`.
        "force_reinforce" => {
            let (id, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <account id>:<bodies>"))?;
            PlayerCommand::ForceReinforce {
                id: id.parse::<u32>().map_err(|e| format!("{spec}: bad account id: {e}"))?,
                bodies: n.parse::<u32>().map_err(|e| format!("{spec}: bad bodies: {e}"))?,
            }
        }
        "outside_order" => {
            let parts: Vec<&str> = value.split(':').collect();
            let [id, order, days] = parts[..] else {
                return Err(format!("{spec}: expected <account id>:<order>:<days>"));
            };
            PlayerCommand::SetOutsideOrder {
                id: id.parse::<u32>().map_err(|e| format!("{spec}: bad account id: {e}"))?,
                order: citysim::outside::OutsideOrder::parse(order)
                    .ok_or_else(|| format!("{spec}: unknown order {order}"))?,
                days: days.parse::<u32>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            }
        }
        "export_cap" => {
            let (g, n) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <food|parts|data>:<n|auto>"))?;
            PlayerCommand::SetExportCap {
                good: citysim::outside::ExportGood::parse(g).ok_or_else(|| format!("{spec}: unknown good {g}"))?,
                cap: if n.eq_ignore_ascii_case("auto") {
                    None
                } else {
                    Some(n.parse::<u32>().map_err(|e| format!("{spec}: bad cap: {e}"))?)
                },
            }
        }
        "ask" => {
            let (g, m) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <food|parts|data>:<mult>"))?;
            PlayerCommand::SetImportAsk {
                good: citysim::outside::ExportGood::parse(g).ok_or_else(|| format!("{spec}: unknown good {g}"))?,
                mult: m.parse::<f32>().map_err(|e| format!("{spec}: bad mult: {e}"))?,
            }
        }
        "customs" => PlayerCommand::SetCustoms(num("rate")? as f32),
        // M16a phase 4 (plan C39): `fixer_licence=on|off`, `accessory_mult=<f>`,
        // `public_bounties=on|off`.
        "fixer_licence" => PlayerCommand::SetFixerLicence(
            on_off(value).ok_or_else(|| format!("{spec}: fixer_licence must be on|off"))?,
        ),
        "accessory_mult" => PlayerCommand::SetAccessoryMult(num("mult")? as f32),
        "public_bounties" => PlayerCommand::PublicBounties(
            on_off(value).ok_or_else(|| format!("{spec}: public_bounties must be on|off"))?,
        ),
        // Jobs and room J19 (P6): the Harris-Todaro outside wage.
        "outside_wage" => PlayerCommand::SetOutsideWage(num("coins")? as f32),
        "close_world" => {
            PlayerCommand::CloseWorld(on_off(value).ok_or_else(|| format!("{spec}: close_world must be on|off"))?)
        }
        "hack_sentence" => {
            let (c, d) = value.split_once(':').ok_or_else(|| format!("{spec}: expected <crime>:<days>"))?;
            let crime = match c.to_ascii_lowercase().as_str() {
                "intrusion" => citysim::Crime::Intrusion,
                "data_theft" | "datatheft" => citysim::Crime::DataTheft,
                _ => return Err(format!("{spec}: crime must be intrusion|data_theft")),
            };
            PlayerCommand::SetHackSentence {
                crime,
                days: d.parse::<u16>().map_err(|e| format!("{spec}: bad days: {e}"))?,
            }
        }
        other => return Err(format!("{spec}: unknown lever {other}")),
    };
    Ok((day * TICKS_PER_DAY, Lever::Cmd(cmd)))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use citysim::{AssetClass, AssetKind, DistrictId, Slot};

    fn cmd(spec: &str) -> PlayerCommand {
        match parse_lever(spec).expect("parses") {
            (_, Lever::Cmd(c)) => c,
            (_, other) => panic!("{spec}: expected a plain command, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_stims_legal() {
        assert_eq!(cmd("day=30:stims_legal=on"), PlayerCommand::SetStimsLegal(true));
        assert_eq!(cmd("day=30:stims_legal=off"), PlayerCommand::SetStimsLegal(false));
        assert!(parse_lever("day=30:stims_legal=maybe").is_err());
    }

    /// P10: the policy levers.
    #[test]
    fn test_parse_policy_levers() {
        assert_eq!(cmd("day=10:levy=on"), PlayerCommand::SetLevy(true));
        assert_eq!(cmd("day=10:school_meals=off"), PlayerCommand::SetSchoolMeals(false));
        assert_eq!(cmd("day=10:reserve_release=on"), PlayerCommand::SetReserveRelease(true));
        assert_eq!(cmd("day=10:safety_net=on"), PlayerCommand::SetSafetyNet(true));
        assert_eq!(cmd("day=10:dole_per_day=4"), PlayerCommand::SetDolePerDay(4));
        assert!(parse_lever("day=10:safety_net=maybe").is_err());
    }

    #[test]
    fn test_parse_asset_tax() {
        let (t, _) = parse_lever("day=5:asset_tax=car:0.5").unwrap();
        assert_eq!(t, 5 * TICKS_PER_DAY);
        assert_eq!(cmd("day=5:asset_tax=car:0.5"), PlayerCommand::SetAssetTax { kind: AssetClass::Car, rate: 0.5 });
        assert_eq!(
            cmd("day=5:asset_tax=chrome:1"),
            PlayerCommand::SetAssetTax { kind: AssetClass::Implant, rate: 1.0 }
        );
        assert!(parse_lever("day=5:asset_tax=boat:1").is_err());
        assert!(parse_lever("day=5:asset_tax=car").is_err());
    }

    #[test]
    fn test_parse_impound() {
        assert_eq!(cmd("day=1:impound=off"), PlayerCommand::SetImpound(false));
        assert_eq!(cmd("day=1:impound=on"), PlayerCommand::SetImpound(true));
        assert!(parse_lever("day=1:impound=2").is_err());
    }

    #[test]
    fn test_parse_grant_asset() {
        match parse_lever("day=2:grant_asset=17:arms:2").unwrap().1 {
            Lever::GrantAsset(17, AssetKind::Implant(Slot::Arms), 2) => {}
            other => panic!("{other:?}"),
        }
        match parse_lever("day=2:grant_asset=3:flyer:1").unwrap().1 {
            Lever::GrantAsset(3, AssetKind::Flyer, 1) => {}
            other => panic!("{other:?}"),
        }
        assert!(parse_lever("day=2:grant_asset=3:tank:1").is_err());
        assert!(parse_lever("day=2:grant_asset=3:car").is_err());
    }

    #[test]
    fn test_parse_wreck() {
        assert!(matches!(parse_lever("day=2:wreck=2100").unwrap().1, Lever::Wreck(2100)));
        assert!(parse_lever("day=2:wreck=x").is_err());
    }

    #[test]
    fn test_parse_chrome_everyone() {
        assert_eq!(cmd("day=10:chrome_everyone=2"), PlayerCommand::ChromeEveryone { tier: 2 });
        assert!(parse_lever("day=10:chrome_everyone=-1").is_err());
    }

    #[test]
    fn test_parse_flood_stims() {
        assert_eq!(cmd("day=10:flood_stims=7:500"), PlayerCommand::FloodStims { district: DistrictId(7), n: 500 });
        assert!(parse_lever("day=10:flood_stims=7").is_err());
    }

    #[test]
    fn test_parse_brick() {
        assert!(matches!(parse_lever("day=10:brick=8").unwrap().1, Lever::Brick(Lender::Corp(8))));
        assert!(matches!(parse_lever("day=10:brick=agent:42").unwrap().1, Lever::Brick(Lender::Agent(42))));
        assert!(parse_lever("day=10:brick=agent:x").is_err());
    }

    #[test]
    fn test_parse_virt_god_levers() {
        assert!(matches!(parse_lever("day=3:wipe_data=812"), Ok((_, Lever::WipeData(812)))));
        assert!(matches!(
            parse_lever("day=3:set_tech=8:chrome:2"),
            Ok((_, Lever::SetTech(8, citysim::virt::Track::Chrome, 2)))
        ));
        assert!(matches!(
            parse_lever("day=3:grant_data=gang1:deck:500"),
            Ok((_, Lever::GrantData(Faction::Gang(1), citysim::virt::Track::Deck, 500)))
        ));
        assert!(parse_lever("day=3:set_tech=8:psionics:2").is_err());
    }

    #[test]
    fn test_parse_virt_levers_phase_4() {
        use citysim::virt::Purpose;
        assert!(matches!(parse_lever("day=3:city_ice=3").unwrap().1, Lever::Cmd(PlayerCommand::SetCityIce(3))));
        assert!(matches!(
            parse_lever("day=3:data_tax=0.25").unwrap().1,
            Lever::Cmd(PlayerCommand::SetDataTax(t)) if (t - 0.25).abs() < 1e-6
        ));
        assert!(matches!(
            parse_lever("day=3:hack_sentence=intrusion:9").unwrap().1,
            Lever::Cmd(PlayerCommand::SetHackSentence { crime: citysim::Crime::Intrusion, days: 9 })
        ));
        assert!(matches!(
            parse_lever("day=3:hack_sentence=data_theft:20").unwrap().1,
            Lever::Cmd(PlayerCommand::SetHackSentence { crime: citysim::Crime::DataTheft, days: 20 })
        ));
        assert!(parse_lever("day=3:hack_sentence=theft:20").is_err());
        assert!(matches!(parse_lever("day=3:grant_deck=77:3").unwrap().1, Lever::GrantDeck(77, 3)));
        assert!(matches!(parse_lever("day=3:set_ice=812:2").unwrap().1, Lever::SetIce(812, 2)));
        assert!(matches!(parse_lever("day=3:fry=77").unwrap().1, Lever::Fry(77)));
        assert!(matches!(
            parse_lever("day=3:run_now=77:812:wipe").unwrap().1,
            Lever::RunNow(77, RunTarget::Building(812), Purpose::Data { wipe: true })
        ));
        assert!(matches!(
            parse_lever("day=3:run_now=77:corp3:ledger").unwrap().1,
            Lever::RunNow(77, RunTarget::Corp(3), Purpose::Ledger)
        ));
        assert!(parse_lever("day=3:run_now=77:812:hax").is_err());
    }

    #[test]
    fn test_parse_virt_god_plurals_phase_5() {
        assert!(matches!(parse_lever("day=20:wipe_corp_data=8").unwrap().1, Lever::WipeCorpData(8)));
        assert!(matches!(parse_lever("day=45:grant_decks=6:10:3").unwrap().1, Lever::GrantDecks(6, 10, 3)));
        assert!(matches!(parse_lever("day=45:set_corp_ice=6:0").unwrap().1, Lever::SetCorpIce(6, 0)));
        assert!(parse_lever("day=45:grant_decks=6:10").is_err());
        assert!(parse_lever("day=45:set_corp_ice=x:0").is_err());
        assert!(matches!(
            parse_lever("day=10:plant_rumour=12:killed:none:3:1.0").unwrap().1,
            Lever::PlantRumour(12, citysim::word::Deed::Killed, None, 3, _)
        ));
        assert!(matches!(
            parse_lever("day=10:set_reputation=12:dread:0.9:5").unwrap().1,
            Lever::SetReputation(12, citysim::word::Axis::Dread, _, 5)
        ));
        assert!(parse_lever("day=10:plant_rumour=12:kissed:none:3:1.0").is_err());
        assert!(matches!(
            parse_lever("day=5:grant_skill=12:persuasion:1.0").unwrap().1,
            Lever::GrantSkill(SkillTarget::Agent(12), citysim::word::SocialSkill::Persuasion, _, false)
        ));
        assert!(matches!(
            parse_lever("day=5:grant_skill=dregs10:persuasion:1.0:suit").unwrap().1,
            Lever::GrantSkill(SkillTarget::Dregs(10), citysim::word::SocialSkill::Persuasion, _, true)
        ));
        assert!(matches!(
            parse_lever("day=5:set_creed=1:purist").unwrap().1,
            Lever::SetCreed(1, Some(citysim::word::Creed::Purist))
        ));
        assert!(parse_lever("day=5:set_creed=1:monk").is_err());
        assert!(parse_lever("day=5:grant_skill=12:charisma:1.0").is_err());
        assert!(parse_lever("day=3:city_ice=x").is_err());
    }

    #[test]
    fn test_parse_word_levers_phase_4() {
        use citysim::PlayerCommand as P;
        assert!(matches!(parse_lever("day=3:press_licence=off").unwrap().1, Lever::Cmd(P::SetPressLicence(false))));
        assert!(matches!(parse_lever("day=3:press_licence=on").unwrap().1, Lever::Cmd(P::SetPressLicence(true))));
        assert!(parse_lever("day=3:press_licence=maybe").is_err());
        match parse_lever("day=3:news_tax=0.2").unwrap().1 {
            Lever::Cmd(P::SetNewsTax(r)) => assert!((r - 0.2).abs() < 1e-6),
            other => panic!("{other:?}"),
        }
        assert!(parse_lever("day=3:news_tax=lots").is_err());
        assert!(matches!(parse_lever("day=3:censor=gang1").unwrap().1, Lever::Censor(FactionRef::Gang(1), true)));
        assert!(matches!(parse_lever("day=3:censor=corp4:off").unwrap().1, Lever::Censor(FactionRef::Corp(4), false)));
        assert!(matches!(parse_lever("day=3:censor=law").unwrap().1, Lever::Censor(FactionRef::Law, true)));
        assert!(parse_lever("day=3:censor=church").is_err());
        assert!(parse_lever("day=3:censor=law:maybe").is_err());
    }

    #[test]
    fn test_parse_word_god_phase_3() {
        match parse_lever("day=10:declare_vendetta=corp6:gang0:1.0").unwrap().1 {
            Lever::DeclareVendetta(FactionRef::Corp(6), FactionRef::Gang(0), w) => assert!((w - 1.0).abs() < 1e-6),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            parse_lever("day=10:declare_vendetta=law:gang1:0.5").unwrap().1,
            Lever::DeclareVendetta(FactionRef::Law, FactionRef::Gang(1), _)
        ));
        assert!(parse_lever("day=10:declare_vendetta=gang0:1.0").is_err());
        assert!(parse_lever("day=10:declare_vendetta=gang0:club2:1.0").is_err());
        assert!(matches!(
            parse_lever("day=10:kill_friend=leader0:member1").unwrap().1,
            Lever::KillFriend(Who::Leader(0), Who::Member(1))
        ));
        assert!(matches!(
            parse_lever("day=10:kill_friend=12:34").unwrap().1,
            Lever::KillFriend(Who::Agent(12), Who::Agent(34))
        ));
        assert!(parse_lever("day=10:kill_friend=leaderx:1").is_err());
        assert!(matches!(parse_lever("day=10:hunt=12:34").unwrap().1, Lever::Hunt(12, 34)));
        assert!(parse_lever("day=10:hunt=12").is_err());
    }

    #[test]
    fn test_parse_contract_levers() {
        use citysim::contract::ContractKind;
        assert!(matches!(
            parse_lever("day=3:post_contract=12:hit:34:700:1:10").unwrap().1,
            Lever::PostContract(Buyer::Agent(12), ContractKind::Hit, ContractTarget::Agent(34), 700, true, 10)
        ));
        assert!(matches!(
            parse_lever("day=3:post_contract=corp2:guard:b77:0:0:7").unwrap().1,
            Lever::PostContract(Buyer::Corp(2), ContractKind::Guard, ContractTarget::Building(77), 0, false, 7)
        ));
        assert!(matches!(
            parse_lever("day=3:post_contract=city:locate:9:0:0:0").unwrap().1,
            Lever::PostContract(Buyer::City, ContractKind::Locate, ContractTarget::Agent(9), 0, false, 0)
        ));
        assert!(parse_lever("day=3:post_contract=12:kill:34:700:1:10").is_err());
        assert!(matches!(parse_lever("day=4:take_contract=5:21").unwrap().1, Lever::TakeContract(5, 21)));
    }

    /// M16b phase 4 (spec § 8): the 16b god levers parse.
    #[test]
    fn test_parse_m16b_levers() {
        assert!(matches!(
            parse_lever("day=5:prison_permit=off").unwrap().1,
            Lever::Cmd(PlayerCommand::SetPrisonPermit(false))
        ));
        assert!(matches!(parse_lever("day=5:expose_secret=7").unwrap().1, Lever::Cmd(PlayerCommand::ExposeSecret(7))));
        assert!(matches!(parse_lever("day=5:call_vote=3").unwrap().1, Lever::M16b(M16bLever::CallVote(3))));
        assert!(matches!(parse_lever("day=5:wound=12").unwrap().1, Lever::M16b(M16bLever::Wound(12))));
        assert!(matches!(parse_lever("day=5:subscribe=12:4").unwrap().1, Lever::M16b(M16bLever::Subscribe(12, 4))));
        assert!(matches!(
            parse_lever("day=5:tag_agent=12:gang1:2").unwrap().1,
            Lever::M16b(M16bLever::TagAgent(12, FactionRef::Gang(1), 2))
        ));
        assert!(matches!(
            parse_lever("day=5:hostage=12:gang0:300:release44").unwrap().1,
            Lever::M16b(M16bLever::Hostage {
                captive: 12,
                holder: Some(FactionRef::Gang(0)),
                at: 300,
                demand: DemandSpec::Release(44)
            })
        ));
        assert!(matches!(
            parse_lever("day=5:coerce=12:13:threat:vote2").unwrap().1,
            Lever::M16b(M16bLever::Coerce(12, 13, LevSpec::Threat, DemandSpec::Vote(2)))
        ));
        assert!(parse_lever("day=5:coerce=12:13:threat:none").is_err());
        assert!(parse_lever("day=5:hostage=12").is_err());
    }

    #[test]
    fn test_parse_contract_board_levers() {
        assert!(matches!(
            parse_lever("day=20:fixer_licence=off").unwrap().1,
            Lever::Cmd(PlayerCommand::SetFixerLicence(false))
        ));
        assert!(matches!(
            parse_lever("day=20:public_bounties=on").unwrap().1,
            Lever::Cmd(PlayerCommand::PublicBounties(true))
        ));
        assert!(matches!(
            parse_lever("day=20:accessory_mult=1.5").unwrap().1,
            Lever::Cmd(PlayerCommand::SetAccessoryMult(m)) if (m - 1.5).abs() < 1e-6
        ));
        assert!(parse_lever("day=20:fixer_licence=maybe").is_err());
    }

    #[test]
    fn test_parse_chase() {
        assert!(matches!(parse_lever("day=10:chase=99").unwrap().1, Lever::Chase(99)));
        assert!(parse_lever("day=10:chase=").is_err());
    }
}
