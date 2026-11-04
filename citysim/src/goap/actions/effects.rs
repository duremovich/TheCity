use super::*;

impl ActionKind {
    /// The "Who" column: may this agent take the action at all?
    pub fn allowed(self, ctx: &PlanCtx) -> bool {
        if !self.implemented() {
            return false;
        }
        match self {
            ActionKind::FarmWork => ctx.is(Role::Farmer),
            // Appended by the executor after FarmWork; never planned.
            ActionKind::HaulToMarket => false,
            ActionKind::ClerkWork => {
                ctx.is(Role::Clerk)
                    || ctx.is(Role::Ripperdoc)
                    || ctx.is(Role::Mechanic)
                    || ctx.is(Role::Researcher)
                    || ctx.is(Role::Reporter)
                    // L2 L2: the six leisure roles.
                    || ctx.is(Role::Host)
                    || ctx.is(Role::Attendant)
                    || ctx.is(Role::Cook)
                    || ctx.is(Role::Fighter)
                    || ctx.is(Role::Croupier)
                    || ctx.is(Role::Concierge)
                    // M16a (plan C8): the Fixer's staff.
                    || ctx.is(Role::Fixer)
                    // Jobs and room J13: every trade.
                    || ctx.role.is_some_and(Role::is_trade)
            }
            ActionKind::BartendWork => ctx.is(Role::Bartender),
            ActionKind::GuardJail => ctx.is(Role::Guard) && ctx.jail_day,
            // L2 L8: a sweeper tends graves only with Sweep off.
            ActionKind::TendGraves => ctx.is(Role::Gravedigger) || (ctx.is(Role::Sanitation) && !ctx.sweep),
            ActionKind::FabWork => ctx.is(Role::Fabber),
            ActionKind::Sweep => ctx.is(Role::Sanitation) && ctx.sweep,
            ActionKind::CarryCorpse | ActionKind::BuryCorpse => ctx.adult && ctx.may_bury,
            ActionKind::CollectWage => ctx.role.is_some(),
            ActionKind::CollectDole => ctx.role.is_none() && ctx.adult,
            ActionKind::Beg => !ctx.is(Role::Guard),
            ActionKind::Extort | ActionKind::SplitLoot => ctx.in_gang,
            // M13 D26: anyone fences a stolen vehicle at a Hideout: one
            // bound to steal, or (review fix) one already held.
            ActionKind::Fence => ctx.in_gang || ctx.vehicle_target || ctx.holds_stolen_vehicle,
            // M13 review: each M13 action is offered only to the goals it
            // serves (see `PlanCtx::goal`).
            ActionKind::BuyAsset | ActionKind::Install => ctx.adult && ctx.serves(&[GoalKind::Shop]),
            // M14 V36: a deck upgrade is a Shop pick too.
            ActionKind::UpgradeDeck => ctx.adult && ctx.serves(&[GoalKind::Shop]),
            // M14 V11/V29: the Hack goal's chain.
            ActionKind::JackIn | ActionKind::SellData => ctx.adult && ctx.serves(&[GoalKind::Hack]),
            ActionKind::StealVehicle => {
                ctx.adult
                    && ctx.lawfulness < 0.4 + ctx.misery_lawful_cut
                    && !ctx.is(Role::Guard)
                    && ctx.serves(&[GoalKind::Earn])
            }
            ActionKind::Therapy | ActionKind::Uninstall | ActionKind::Detox => {
                ctx.adult && ctx.serves(&[GoalKind::Treat])
            }
            ActionKind::Strip => ctx.adult && ctx.serves(&[GoalKind::Loot]),
            ActionKind::Rip => ctx.adult && ctx.may_rip && ctx.serves(&[GoalKind::Loot, GoalKind::GangWork]),
            ActionKind::Abduct => ctx.in_gang && ctx.serves(&[GoalKind::GangWork]),
            // M13 D38/D39.
            ActionKind::PickUp | ActionKind::Deal => ctx.in_gang && ctx.dealer && ctx.serves(&[GoalKind::GangWork]),
            ActionKind::BuyStims | ActionKind::UseStim => ctx.adult && ctx.serves(&[GoalKind::GetHigh]),
            // M12 D31: a rioter marches too.
            ActionKind::Muster | ActionKind::Brawl => ctx.in_gang || ctx.raid_pending,
            ActionKind::PatrolLeg | ActionKind::Arrest | ActionKind::Escort => ctx.is(Role::Guard),
            ActionKind::JoinGang => !ctx.in_gang && !ctx.is(Role::Guard) && ctx.adult && ctx.gang_eligible,
            ActionKind::Flirt | ActionKind::Propose => ctx.adult,
            ActionKind::Register => ctx.adult && !ctx.in_gang,
            ActionKind::CheckIn => ctx.adult && (ctx.homeless || ctx.away_hotel),
            // L1: the broke's Earn (off: never).
            ActionKind::Scavenge => ctx.adult && ctx.poor && ctx.serves(&[GoalKind::Earn]),
            ActionKind::Meeting => false,
            ActionKind::Occupy => ctx.adult && (ctx.homeless || ctx.gang_squat),
            ActionKind::ServeTime => false,
            // L2 L14: scripted only.
            k if k.is_leisure_step() => false,
            // M16a (plan C10): scripted only.
            k if k.is_contract_step() => false,
            // Real economy E27, E28: scripted only.
            k if k.is_charity_step() => false,
            ActionKind::DumpsterDive
            | ActionKind::Oncology
            | ActionKind::Patch
            | ActionKind::PackMeal
            | ActionKind::Vend
            | ActionKind::Break
            | ActionKind::Unload
            | ActionKind::Court => false,
            _ => true,
        }
    }

    /// M18 P5 (Dylan 2026-11-03, autopilot is safe idle): a step the
    /// player's body on autopilot may take. It eats, sleeps, works the job it
    /// holds, begs and keeps out of trouble; it never steals, fights, joins,
    /// deals, courts, signs or buys its way into a new life.
    pub fn safe_idle_ok(self) -> bool {
        !matches!(
            self,
            ActionKind::StealFood(_)
                | ActionKind::StealVehicle
                | ActionKind::Fence
                | ActionKind::Attack
                | ActionKind::Brawl
                | ActionKind::Muster
                | ActionKind::JoinGang
                | ActionKind::Extort
                | ActionKind::SplitLoot
                | ActionKind::Strip
                | ActionKind::Rip
                | ActionKind::Abduct
                | ActionKind::Deal
                | ActionKind::BuyStims
                | ActionKind::UseStim
                | ActionKind::Flirt
                | ActionKind::Propose
                | ActionKind::Register
                | ActionKind::Occupy
                | ActionKind::BuyAsset
                | ActionKind::Install
                | ActionKind::JackIn
                | ActionKind::SellData
                | ActionKind::UpgradeDeck
                | ActionKind::ReportCrime
                // M18 P5h: safe idle never signs a lease.
                | ActionKind::Apply
        )
    }

    /// Symbolic preconditions against the observed state.
    pub fn preconditions(self, ws: &WorldState, ctx: &PlanCtx) -> bool {
        if ctx.safe_idle && !self.safe_idle_ok() {
            return false;
        }
        let at = |k: LocationKey| ws.at == k;
        match self {
            ActionKind::GoTo(k) => ws.at != k && ctx.dist.contains_key(&k),
            ActionKind::EatFromInventory => ws.has_food,
            ActionKind::EatAtHome => at(LocationKey::Home) && ctx.pantry > 0,
            ActionKind::BuyFood => at(LocationKey::Market) && ws.has_coins && ctx.market_free,
            ActionKind::StealFood(StealSource::Market) => {
                at(LocationKey::Market) && !ws.carrying_stolen && ctx.market_free
            }
            ActionKind::StealFood(StealSource::Home) => {
                at(LocationKey::TargetHome) && !ws.carrying_stolen && ctx.target_pantry > 0
            }
            ActionKind::StealFood(StealSource::Warehouse) => {
                at(LocationKey::Warehouse) && !ws.carrying_stolen && ctx.warehouse_stock > 0 && ctx.stealth >= 0.4
            }
            ActionKind::Forage => at(LocationKey::Farm) && ws.forage_available,
            // `bar_full` is a plan-time hint only: once inside, the agent counts as an occupant.
            ActionKind::Beg => {
                at(LocationKey::Market)
                    || (at(LocationKey::Bar) && !ctx.bar_full)
                    || (ctx.beg_spot && at(LocationKey::Street))
            }
            // A member on watch, lying low or homeless beds down at the Hideout.
            // M12 D21: one who can afford a bed never plans the street; a
            // booked guest sleeps at the Hotel, a squatter in the squat.
            ActionKind::Sleep => {
                (at(LocationKey::Home) && !ctx.holes_up)
                    || (ctx.homeless && !ctx.hotel_available && !ctx.squatter && at(LocationKey::Street))
                    || (ctx.holes_up && at(LocationKey::Hideout))
                    || (ws.checked_in && (at(LocationKey::Hotel) || at(LocationKey::Cot)))
                    || (ctx.squatter && at(LocationKey::Squat))
                    // L1: the Hideout nearer than Home; lying down where one stands.
                    || (ctx.hideout_bed && at(LocationKey::Hideout))
                    || (ctx.rough_ok && at(LocationKey::Street))
            }
            // Rest at Bar or Home; also at the workplace while waiting for a
            // shift, and at the Hideout for a gang member.
            ActionKind::Rest => {
                (at(LocationKey::Bar) && !ctx.bar_full)
                    || at(LocationKey::Home)
                    || ctx.workplace.is_some_and(at)
                    || (ctx.in_gang && at(LocationKey::Hideout))
            }
            ActionKind::FarmWork => at(LocationKey::Farm) && !ws.shift_done && ctx.on_shift,
            // M13 D16: Ripperdocs and Mechanics clerk at their workplace.
            ActionKind::ClerkWork => {
                (if ctx.is(Role::Clerk) { at(LocationKey::Market) } else { ctx.workplace.is_some_and(at) })
                    && !ws.shift_done
                    && ctx.on_shift
            }
            ActionKind::BartendWork => at(LocationKey::Bar) && !ws.shift_done && (ctx.on_shift || ctx.evening),
            ActionKind::GuardJail => at(LocationKey::Jail) && !ws.shift_done && ctx.on_shift,
            ActionKind::TendGraves => at(LocationKey::Cemetery) && !ws.shift_done && ctx.on_shift,
            ActionKind::FabWork => ctx.workplace.is_some_and(at) && !ws.shift_done && ctx.on_shift,
            // L2 L8: on the beat (`observe` reports `Beat` on its streets under Work).
            ActionKind::Sweep => at(LocationKey::Beat) && !ws.shift_done && ctx.on_shift,
            ActionKind::HaulToMarket => at(LocationKey::Farm) && ctx.farm_stock >= ctx.haul_min_stock,
            ActionKind::CollectWage => at(ctx.wage_at) && ws.has_wage_due && ctx.wage_collectable,
            ActionKind::CollectDole => at(LocationKey::Hall) && ctx.dole_available,
            ActionKind::SellFood => at(LocationKey::Market) && ws.has_food && !ws.carrying_stolen,
            // L2 L14: also inside a Club or Den with leisure on (`leisure_drink`).
            ActionKind::Drink => {
                (at(LocationKey::Bar) && ctx.coins >= 2 && !ctx.drank_today && !ctx.bar_full)
                    || (ctx.leisure_drink && ctx.coins >= 2 && !ctx.drank_today)
            }
            ActionKind::StoreFood => at(LocationKey::Home) && ctx.food_unstolen,
            ActionKind::Wander => true,
            ActionKind::ReportCrime => at(LocationKey::Hall) && !ws.crime_reported && ctx.hall_open,
            ActionKind::PatrolLeg => at(LocationKey::PatrolWaypoint) && ctx.patrol_pending,
            ActionKind::Arrest => at(LocationKey::SuspectTile) && ws.known_suspect_location && !ws.suspect_cuffed,
            ActionKind::Escort => ws.suspect_cuffed && ctx.dist.contains_key(&LocationKey::Jail),
            ActionKind::HideFromLaw => (at(LocationKey::Hideout) || at(LocationKey::Home)) && ctx.wanted,
            ActionKind::FleeToHome => {
                !ws.is_safe
                    && (ctx.dist.contains_key(&LocationKey::Home)
                        || ctx.refuge.is_some_and(|k| ctx.dist.contains_key(&k)))
            }
            ActionKind::Scavenge => at(LocationKey::Street) && ctx.poor,
            ActionKind::Meeting => true,
            // L2 L14: the step re-check is `leisure::can_start` (through
            // `actions::can_start`), not the planner's symbols.
            k if k.is_leisure_step() => true,
            // M16a (plan C10): likewise `contracts::can_start`.
            k if k.is_contract_step() => true,
            // Real economy E27, E28: likewise `charity::can_start`.
            k if k.is_charity_step() => true,
            ActionKind::DumpsterDive
            | ActionKind::Oncology
            | ActionKind::Patch
            | ActionKind::PackMeal
            | ActionKind::Vend
            | ActionKind::Break
            | ActionKind::Unload
            | ActionKind::Court => true,
            // M18 P3: or on the street beside the partner (`street_partner`).
            ActionKind::Chat => {
                (matches!(ws.at, LocationKey::Market | LocationKey::Bar | LocationKey::Home | LocationKey::Farm)
                    || (ws.at == LocationKey::Street && ctx.street_partner))
                    && ctx.partner.is_some()
            }
            // Flirt and Propose plan toward a partner who is here or at a Bar /
            // Market; execution re-checks co-location (PartnerLeft otherwise).
            ActionKind::Flirt => {
                matches!(ws.at, LocationKey::Bar | LocationKey::Market) && ctx.partner_reachable && !ws.has_spouse
            }
            ActionKind::Propose => ws.has_partner_candidate && ctx.partner_reachable && !ws.has_spouse,
            ActionKind::JoinGang => at(LocationKey::Hideout) && !ws.in_gang,
            ActionKind::Extort => ws.in_gang && at(LocationKey::TargetHome) && !ctx.guard8 && ctx.extort_ok,
            ActionKind::SplitLoot => {
                ws.in_gang && at(LocationKey::Hideout) && ws.gang_task_done && ctx.has_loot && !ctx.hideout_sacked
            }
            ActionKind::Fence => {
                at(LocationKey::Hideout)
                    && !ctx.hideout_sacked
                    && ((ctx.in_gang && ws.carrying_stolen && ctx.can_fence)
                        || (ws.carrying_vehicle && ctx.can_fence_vehicle))
            }
            ActionKind::BuyAsset => {
                at(LocationKey::Seller) && ctx.shop_pick && !ctx.install_ready && !ctx.shop_upgrade && !ws.bought
            }
            ActionKind::UpgradeDeck => at(LocationKey::Seller) && ctx.shop_pick && ctx.shop_upgrade && !ws.bought,
            ActionKind::JackIn => at(LocationKey::Chair) && ctx.run_order && !ws.run_done,
            ActionKind::SellData => at(LocationKey::DataBuyer) && ws.has_data,
            // M13 D34: a gang implant is installed where it is bought: at the seller.
            ActionKind::Install => at(LocationKey::Seller) && ctx.install_ready && !ws.bought,
            ActionKind::Therapy => at(LocationKey::Clinic) && ctx.therapy_affordable && !ws.treated,
            ActionKind::Uninstall => at(LocationKey::Clinic) && ctx.has_implant && !ws.treated,
            ActionKind::Strip => at(LocationKey::CorpseTile) && ctx.loot_target && !ws.stripped,
            // "Strip [-> Rip]": a body is ripped once it is stripped (the
            // executor appends the Rip to a ripper's Strip).
            ActionKind::Rip => {
                (at(LocationKey::CorpseTile) && ctx.loot_implants && !ctx.loot_target)
                    || (at(LocationKey::Hideout) && ws.carrying_corpse)
            }
            ActionKind::Abduct => at(LocationKey::Victim) && ctx.victim_target && !ws.carrying_corpse,
            ActionKind::StealVehicle => at(LocationKey::Vehicle) && ctx.vehicle_target && !ws.carrying_vehicle,
            // M13 D38/D39.
            ActionKind::PickUp => at(LocationKey::Hideout) && ctx.hideout_stims >= ctx.deal_batch && !ws.has_stims,
            ActionKind::Deal => at(LocationKey::Seller) && ws.has_stims && !ws.gang_task_done,
            ActionKind::BuyStims => at(LocationKey::StimSource) && ctx.stim_affordable && !ws.has_stims,
            ActionKind::UseStim => ws.has_stims && !ws.high,
            ActionKind::Detox => at(LocationKey::Clinic) && ctx.detox_affordable && !ws.treated,
            ActionKind::Muster => at(LocationKey::MusterPoint) && !ws.mustered && ctx.raid_pending,
            ActionKind::Brawl => at(LocationKey::RaidTarget) && ws.mustered && !ws.raid_done,
            // M13 D33: a berserker closes in on its quarry first.
            ActionKind::Attack => {
                (ctx.hostile_adjacent || (ctx.episode && at(LocationKey::Victim))) && !ws.threat_removed
            }
            ActionKind::CarryCorpse => at(LocationKey::CorpseTile) && ws.known_corpse && !ws.carrying_corpse,
            ActionKind::BuryCorpse => at(LocationKey::Cemetery) && ws.carrying_corpse,
            ActionKind::Register => at(LocationKey::Hall) && !ws.founded && ctx.can_found,
            ActionKind::CheckIn => {
                (at(LocationKey::Hotel) || at(LocationKey::Cot)) && ctx.hotel_available && !ws.checked_in
            }
            ActionKind::Occupy => {
                (at(LocationKey::Squat) || at(LocationKey::TargetHome))
                    && ctx.squat_ok
                    && (ctx.gang_squat || !ws.squatting)
            }
            _ => false,
        }
    }

    /// Could this action ever run for this agent today, ignoring where it is
    /// and the symbolic flags? The context-only half of `preconditions`, used
    /// to fail an unsatisfiable goal before the search exhausts itself.
    pub fn feasible(self, ctx: &PlanCtx) -> bool {
        if !self.allowed(ctx) {
            return false;
        }
        match self {
            ActionKind::GoTo(k) => ctx.dist.contains_key(&k),
            ActionKind::EatAtHome => ctx.pantry > 0,
            ActionKind::BuyFood => ctx.coins >= ctx.price && ctx.market_free,
            ActionKind::StealFood(StealSource::Market) => ctx.market_free,
            ActionKind::StealFood(StealSource::Home) => ctx.target_pantry > 0,
            ActionKind::StealFood(StealSource::Warehouse) => ctx.warehouse_stock > 0 && ctx.stealth >= 0.4,
            ActionKind::Forage => matches!(ctx.season, Season::Summer | Season::Autumn) && !ctx.dark,
            ActionKind::FarmWork
            | ActionKind::ClerkWork
            | ActionKind::GuardJail
            | ActionKind::TendGraves
            | ActionKind::FabWork
            | ActionKind::Sweep => ctx.on_shift,
            ActionKind::BartendWork => ctx.on_shift || ctx.evening,
            ActionKind::HaulToMarket => ctx.farm_stock >= ctx.haul_min_stock,
            ActionKind::CollectWage => ctx.wage_collectable,
            ActionKind::CollectDole => ctx.dole_available,
            ActionKind::Drink => ctx.coins >= 2 && !ctx.drank_today && !ctx.bar_full,
            ActionKind::StoreFood => ctx.food_unstolen,
            ActionKind::ReportCrime => ctx.hall_open,
            ActionKind::PatrolLeg => ctx.patrol_pending && ctx.dist.contains_key(&LocationKey::PatrolWaypoint),
            ActionKind::Arrest => ctx.suspect_located,
            ActionKind::HideFromLaw => ctx.wanted,
            ActionKind::FleeToHome => {
                ctx.dist.contains_key(&LocationKey::Home) || ctx.refuge.is_some_and(|k| ctx.dist.contains_key(&k))
            }
            ActionKind::Scavenge => ctx.poor,
            ActionKind::Chat => ctx.partner.is_some(),
            ActionKind::Flirt | ActionKind::Propose => ctx.partner_reachable,
            ActionKind::JoinGang => ctx.gang_eligible && ctx.dist.contains_key(&LocationKey::Hideout),
            ActionKind::Extort => ctx.extort_ok && ctx.dist.contains_key(&LocationKey::TargetHome),
            ActionKind::SplitLoot => ctx.has_loot && !ctx.hideout_sacked,
            ActionKind::Fence => ((ctx.in_gang && ctx.can_fence) || ctx.can_fence_vehicle) && !ctx.hideout_sacked,
            ActionKind::BuyAsset => ctx.shop_pick && ctx.dist.contains_key(&LocationKey::Seller),
            ActionKind::UpgradeDeck => ctx.shop_pick && ctx.shop_upgrade && ctx.dist.contains_key(&LocationKey::Seller),
            ActionKind::JackIn => ctx.run_order && ctx.dist.contains_key(&LocationKey::Chair),
            ActionKind::SellData => ctx.has_data && ctx.dist.contains_key(&LocationKey::DataBuyer),
            ActionKind::Install => ctx.install_ready && ctx.dist.contains_key(&LocationKey::Seller),
            ActionKind::Therapy => ctx.therapy_affordable && ctx.dist.contains_key(&LocationKey::Clinic),
            ActionKind::Uninstall => ctx.has_implant && ctx.dist.contains_key(&LocationKey::Clinic),
            ActionKind::Strip => ctx.loot_target && ctx.dist.contains_key(&LocationKey::CorpseTile),
            ActionKind::Rip => ctx.loot_implants || ctx.victim_target,
            ActionKind::Abduct => ctx.victim_target && ctx.dist.contains_key(&LocationKey::Victim),
            ActionKind::StealVehicle => ctx.vehicle_target && ctx.dist.contains_key(&LocationKey::Vehicle),
            ActionKind::PickUp => ctx.hideout_stims >= ctx.deal_batch && ctx.dist.contains_key(&LocationKey::Hideout),
            ActionKind::Deal => ctx.dist.contains_key(&LocationKey::Seller),
            ActionKind::BuyStims => ctx.stim_affordable && ctx.dist.contains_key(&LocationKey::StimSource),
            ActionKind::Detox => ctx.detox_affordable && ctx.dist.contains_key(&LocationKey::Clinic),
            ActionKind::Muster => ctx.raid_pending && ctx.dist.contains_key(&LocationKey::MusterPoint),
            ActionKind::Brawl => ctx.raid_pending && ctx.dist.contains_key(&LocationKey::RaidTarget),
            ActionKind::Attack => ctx.hostile_adjacent || (ctx.episode && ctx.dist.contains_key(&LocationKey::Victim)),
            ActionKind::CarryCorpse => ctx.corpse_target,
            ActionKind::BuryCorpse => ctx.corpse_target && ctx.dist.contains_key(&LocationKey::Cemetery),
            ActionKind::Register => ctx.can_found && ctx.dist.contains_key(&LocationKey::Hall),
            ActionKind::CheckIn => {
                ctx.hotel_available
                    && (ctx.dist.contains_key(&LocationKey::Hotel) || ctx.dist.contains_key(&LocationKey::Cot))
            }
            ActionKind::Occupy => ctx.squat_ok,
            _ => true,
        }
    }

    /// Does this action set `key` to `value`? (The symbolic effect, read off
    /// `apply` on a blank state.)
    pub fn produces(self, key: crate::goap::world_state::Key, value: bool, ctx: &PlanCtx) -> bool {
        let blank = WorldState::default();
        let a = self.apply(&blank, ctx);
        // Also from a state where the key is already the opposite, so "set false" shows.
        let mut flipped = blank;
        set_key(&mut flipped, key, !value);
        let b = self.apply(&flipped, ctx);
        a.get(key) == value && b.get(key) == value
    }

    /// Symbolic effects. `coin_bucket` and `food_count` are kept so `has_coins`
    /// and `has_food` after a step are computable from the state alone.
    pub fn apply(self, ws: &WorldState, _ctx: &PlanCtx) -> WorldState {
        let mut n = *ws;
        let eat = |n: &mut WorldState| {
            n.hunger_satisfied = true;
            n.food_count = n.food_count.saturating_sub(1);
            n.has_food = n.food_count >= 1;
        };
        let gain_food = |n: &mut WorldState, units: u8| {
            n.food_count = (n.food_count + units).min(FOOD_COUNT_MAX);
            n.has_food = true;
        };
        let spend = |n: &mut WorldState| {
            n.coin_bucket = n.coin_bucket.saturating_sub(1);
            n.has_coins = n.coin_bucket >= 1;
            n.has_savings = false;
        };
        let income = |n: &mut WorldState| {
            n.coin_bucket = 2;
            n.has_coins = true;
            n.has_savings = true; // the best income available to this agent; see commit notes
        };
        match self {
            ActionKind::GoTo(k) => n.at = k,
            ActionKind::EatFromInventory => eat(&mut n),
            ActionKind::EatAtHome => n.hunger_satisfied = true,
            ActionKind::BuyFood => {
                gain_food(&mut n, 1);
                spend(&mut n);
            }
            ActionKind::StealFood(_) => {
                gain_food(&mut n, 2);
                n.carrying_stolen = true;
            }
            ActionKind::Forage => gain_food(&mut n, 1),
            // L1: for the broke's Earn, a coin or two is the income there is.
            ActionKind::Beg | ActionKind::Scavenge if _ctx.poor && _ctx.goal == Some(GoalKind::Earn) => income(&mut n),
            ActionKind::Beg => {
                // Yields 1-2 coins at best: only a meal's worth when the price is that low.
                if _ctx.price <= 2 {
                    n.coin_bucket = n.coin_bucket.max(1);
                    n.has_coins = true;
                }
            }
            // Rest really gives +0.1 energy: claiming `energy_satisfied` sent tired
            // agents to the Bar to doze instead of home to sleep.
            ActionKind::Sleep => n.energy_satisfied = true,
            ActionKind::Rest => {}
            k if k.is_work() => {
                n.shift_done = true;
                n.has_wage_due = true;
            }
            ActionKind::HaulToMarket => n.at = LocationKey::Market,
            ActionKind::CollectWage => {
                n.has_wage_due = false;
                income(&mut n);
            }
            ActionKind::CollectDole => income(&mut n),
            ActionKind::SellFood => {
                n.has_food = false;
                n.food_count = 0;
                n.coin_bucket = n.coin_bucket.max(1);
                n.has_coins = true;
            }
            ActionKind::Drink => {
                n.belonging_satisfied = true;
                spend(&mut n);
            }
            ActionKind::StoreFood => {
                n.has_food = false;
                n.food_count = 0;
            }
            ActionKind::Wander => n.at = LocationKey::Street,
            ActionKind::ReportCrime => n.crime_reported = true,
            ActionKind::PatrolLeg => n.patrol_leg_done = true,
            ActionKind::Arrest => n.suspect_cuffed = true,
            ActionKind::Escort => {
                n.at = LocationKey::Jail;
                n.suspect_jailed = true;
            }
            ActionKind::HideFromLaw => n.is_safe = true,
            ActionKind::FleeToHome => {
                n.at = LocationKey::Home;
                n.is_safe = true;
            }
            ActionKind::Chat => n.belonging_satisfied = true,
            // Flirt's planning effect is deterministic; the roll happens at execution.
            ActionKind::Flirt => n.has_partner_candidate = true,
            ActionKind::Propose => n.has_spouse = true,
            ActionKind::JoinGang => n.in_gang = true,
            ActionKind::Extort => {
                n.gang_task_done = true;
                n.coin_bucket = n.coin_bucket.max(1);
                n.has_coins = true;
            }
            ActionKind::SplitLoot | ActionKind::Fence => {
                n.has_coins = true;
                n.coin_bucket = 2;
                n.has_savings = true;
                if self == ActionKind::Fence {
                    // A vehicle's fence leaves the food; food's leaves the vehicle.
                    if n.carrying_vehicle {
                        n.carrying_vehicle = false;
                    } else {
                        n.carrying_stolen = false;
                        n.has_food = false;
                        n.food_count = 0;
                    }
                }
            }
            ActionKind::Attack => n.threat_removed = true,
            ActionKind::Muster => n.mustered = true,
            ActionKind::Brawl => n.raid_done = true,
            ActionKind::CarryCorpse => {
                n.carrying_corpse = true;
                n.at = LocationKey::Street;
            }
            ActionKind::BuryCorpse => {
                n.carrying_corpse = false;
                n.corpse_buried = true;
            }
            ActionKind::Register => n.founded = true,
            ActionKind::CheckIn => {
                n.checked_in = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::Occupy => {
                n.squatting = true;
                if _ctx.gang_squat {
                    n.gang_task_done = true;
                }
            }
            ActionKind::BuyAsset => {
                n.bought = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::StealVehicle => n.carrying_vehicle = true,
            ActionKind::Install => {
                n.bought = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::Therapy => {
                n.treated = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::Uninstall => n.treated = true,
            ActionKind::Strip => n.stripped = true,
            ActionKind::Rip => {
                n.stripped = true;
                n.gang_task_done = true;
                n.carrying_corpse = false;
            }
            // The abductee rides the corpse-carry key (plan, Actions table).
            ActionKind::Abduct => n.carrying_corpse = true,
            ActionKind::PickUp => n.has_stims = true,
            ActionKind::Deal => n.gang_task_done = true,
            ActionKind::BuyStims => {
                n.has_stims = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::UseStim => n.high = true,
            ActionKind::JackIn => n.run_done = true,
            ActionKind::SellData => {
                n.has_data = false;
                n.coin_bucket = 2;
                n.has_coins = true;
            }
            ActionKind::UpgradeDeck => {
                n.bought = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            ActionKind::Detox => {
                n.treated = true;
                n.coin_bucket = n.coin_bucket.saturating_sub(1);
                n.has_coins = n.coin_bucket >= 1;
                n.has_savings = false;
            }
            _ => {}
        }
        n
    }

    /// The Action table's cost, clamped to `[0.5, 60]`.
    pub fn cost(self, ctx: &PlanCtx) -> f32 {
        let steal_mods = |base: f32| {
            let mut c = base + ctx.lawfulness * 20.0;
            if ctx.guard8 {
                c += 6.0;
            }
            if ctx.starving {
                c -= 4.0;
            }
            if ctx.dark {
                c -= 3.0;
            }
            // M13 D39: withdrawal makes a theft cheaper (as starving does).
            if ctx.withdrawal_bonus != 0.0 {
                c -= ctx.withdrawal_bonus;
            }
            // The pressure pass phase 1 (OPEN-178): misery steals sooner.
            if ctx.misery_steal_cut != 0.0 {
                c -= ctx.misery_steal_cut;
            }
            c - ctx.stealth * 4.0
        };
        let c = match self {
            ActionKind::GoTo(k) => {
                let d = ctx.dist.get(&k).copied().unwrap_or(0) as f32;
                2.0 + d / 16.0 + if ctx.dark && k == LocationKey::Street { 3.0 } else { 0.0 }
            }
            ActionKind::EatFromInventory => 1.0,
            ActionKind::EatAtHome => 2.0 + if ctx.pantry <= 2 { 4.0 } else { 0.0 },
            ActionKind::BuyFood => 3.0 + ctx.greed * 2.0,
            ActionKind::StealFood(StealSource::Market) => steal_mods(8.0) + ctx.steal_can_buy,
            ActionKind::StealFood(StealSource::Home) => {
                steal_mods(7.0) + if ctx.target_occupied { 5.0 } else { 0.0 } + ctx.steal_can_buy
            }
            ActionKind::StealFood(StealSource::Warehouse) => steal_mods(9.0) + 4.0 + ctx.steal_can_buy,
            ActionKind::Forage => 6.0 + if ctx.hunger < 0.3 { 2.0 } else { 0.0 },
            ActionKind::Beg => {
                let mut c = 5.0 + ctx.pride * 10.0 + (1.0 - ctx.sociability) * 4.0;
                // L2 shadow fixes item 19: a corner with company is an
                // audience: half the cost (else Scavenge always won).
                if ctx.beg_spot {
                    c *= 0.5;
                }
                if ctx.fun_term > 0.0 {
                    c * (1.0 - 0.5 * ctx.fun_term)
                } else {
                    c
                }
            }
            ActionKind::Sleep => 1.0 + if ctx.homeless || ctx.rough_ok { 6.0 } else { 0.0 },
            ActionKind::Rest => 4.0,
            ActionKind::FarmWork => 3.0 - ctx.farming * 2.0,
            ActionKind::HaulToMarket => 3.0,
            ActionKind::ClerkWork
            | ActionKind::BartendWork
            | ActionKind::GuardJail
            | ActionKind::TendGraves
            | ActionKind::FabWork
            | ActionKind::Sweep => 3.0,
            ActionKind::CollectWage => 2.0 + if ctx.payer_negative { 4.0 } else { 0.0 },
            ActionKind::SellFood => 3.0 - ctx.greed * 2.0,
            ActionKind::Fence => 3.0 + ctx.lawfulness * 8.0,
            ActionKind::Chat => 3.0 - ctx.sociability * 2.0,
            ActionKind::Drink => 4.0 - ctx.sociability * 2.0,
            ActionKind::Flirt => 4.0 + (1.0 - ctx.sociability) * 4.0,
            ActionKind::Propose => 5.0 - ctx.courage * 2.0,
            ActionKind::ReportCrime => 2.0 + (1.0 - ctx.lawfulness) * 10.0 + if ctx.in_gang { 8.0 } else { 0.0 },
            ActionKind::PatrolLeg => 2.0,
            ActionKind::Arrest => 4.0 + (1.0 - ctx.courage) * 6.0,
            ActionKind::Escort => 3.0,
            ActionKind::HideFromLaw => 4.0 - (1.0 - ctx.lawfulness) * 2.0,
            ActionKind::FleeToHome => 2.0 - (1.0 - ctx.courage) * 3.0,
            ActionKind::Attack => 10.0 + (1.0 - ctx.courage) * 15.0 + ctx.lawfulness * 10.0 - ctx.fighting * 6.0,
            ActionKind::JoinGang => 6.0 + ctx.lawfulness * 15.0 - (1.0 - ctx.hunger) * 4.0,
            ActionKind::Extort => 6.0 + ctx.lawfulness * 12.0,
            ActionKind::SplitLoot => 2.0,
            ActionKind::Muster => 1.0,
            ActionKind::Brawl => 15.0,
            ActionKind::BuryCorpse => 2.0,
            ActionKind::CarryCorpse => 3.0,
            ActionKind::Wander => 1.0,
            ActionKind::CollectDole => 2.0,
            ActionKind::ServeTime => 60.0,
            ActionKind::StoreFood => 1.0,
            ActionKind::Register => 20.0,
            ActionKind::CheckIn => 4.0,
            ActionKind::Occupy => 6.0,
            ActionKind::BuyAsset => 10.0,
            // M13 D26: StealFood's lawfulness, guard, starving and dark terms.
            ActionKind::StealVehicle => steal_mods(ctx.steal_vehicle_cost),
            ActionKind::Install | ActionKind::Therapy => 5.0,
            ActionKind::Uninstall => 8.0,
            ActionKind::Strip => 4.0 + 4.0 * ctx.lawfulness,
            ActionKind::Rip | ActionKind::Abduct => 6.0,
            ActionKind::PickUp => 2.0,
            ActionKind::Deal => 3.0,
            ActionKind::BuyStims => {
                if ctx.stim_legal {
                    3.0
                } else {
                    3.0 + 3.0 * ctx.lawfulness
                }
            }
            ActionKind::UseStim => 1.0,
            ActionKind::Detox => 5.0,
            // M14 Actions table: 3 + 3 x lawfulness for a freelancer.
            ActionKind::JackIn => 3.0 + 3.0 * ctx.lawfulness,
            ActionKind::SellData => 2.0,
            ActionKind::UpgradeDeck => 6.0,
            // M15: scripted steps, never searched.
            ActionKind::AskAround | ActionKind::StakeOut => 60.0,
            // L1: a pride-weighted chore, dearer than the dole, cheaper than begging.
            ActionKind::Scavenge => {
                let c = 4.0 + ctx.pride * 4.0;
                if ctx.fun_term > 0.0 {
                    c * (1.0 - 0.5 * ctx.fun_term)
                } else {
                    c
                }
            }
            ActionKind::Meeting => 60.0,
            // L2 L14: scripted steps, never searched.
            ActionKind::Enjoy
            | ActionKind::Gamble
            | ActionKind::EatOut
            | ActionKind::HangOut
            | ActionKind::Collect
            | ActionKind::Preach => 60.0,
            // M16a (plan C10, C15): scripted, never planned.
            ActionKind::Network
            | ActionKind::TakeJob
            | ActionKind::Commission
            | ActionKind::Apply
            | ActionKind::HandOver
            | ActionKind::CarryWounded
            | ActionKind::Stabilise => 4.0,
            ActionKind::Guard => 2.0,
            // Real economy E27, E28: scripted, never searched.
            ActionKind::EatAlms
            | ActionKind::Donate
            | ActionKind::DumpsterDive
            | ActionKind::Oncology
            | ActionKind::Patch
            | ActionKind::PackMeal
            | ActionKind::Vend
            | ActionKind::Break
            | ActionKind::Unload
            | ActionKind::Court => 60.0,
        };
        c.clamp(0.5, 60.0)
    }

    /// The entity a step is about, if the kind needs one bound in the plan.
    pub fn instance(self, ctx: &PlanCtx) -> ActionInstance {
        let target = match self {
            ActionKind::GoTo(LocationKey::TargetHome)
            | ActionKind::StealFood(StealSource::Home)
            | ActionKind::GoTo(LocationKey::SuspectTile)
            | ActionKind::Arrest
            | ActionKind::Escort
            | ActionKind::Chat
            | ActionKind::Flirt
            | ActionKind::Propose
            | ActionKind::Extort
            | ActionKind::Attack
            | ActionKind::GoTo(LocationKey::CorpseTile)
            | ActionKind::CarryCorpse
            | ActionKind::BuryCorpse
            | ActionKind::Occupy
            | ActionKind::GoTo(LocationKey::Seller)
            | ActionKind::BuyAsset
            | ActionKind::GoTo(LocationKey::Vehicle)
            | ActionKind::StealVehicle
            | ActionKind::GoTo(LocationKey::Clinic)
            | ActionKind::GoTo(LocationKey::Victim)
            | ActionKind::Strip
            | ActionKind::Rip
            | ActionKind::Abduct
            | ActionKind::Deal
            | ActionKind::GoTo(LocationKey::StimSource)
            | ActionKind::BuyStims
            | ActionKind::UpgradeDeck => ctx.target,
            _ => None,
        };
        ActionInstance { action: self, target, tile: None }
    }
}
