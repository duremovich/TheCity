//! Whole-world serialisation with serde + RON. Everything on `World` is
//! saved except the flow-field cache, which is rebuilt lazily.

use std::path::{Path, PathBuf};

use crate::world::World;

/// The save format, bumped when the saved shape changes (`World::save_version`).
/// 4: the Real economy (the World's books, `wage_rev` and the revenue windows,
/// Missions, Camps, the till). Older saves are not loaded: the legacy
/// migrations retired with the off switches (2026-10-09). 5: Jobs and room P2
/// (`Building.floors`, `floor_days`, `MapBuilding.floors`; serde defaults).
/// 6: Jobs and room P3 (J32): `Role::Trade` by key (`Trade("night_porter")`,
/// resolved against the save's own `Config::trades`; an unknown key fails
/// the load) and the `trades` rows in the saved config. 7: Hunger as
/// sickness (`World.health`, `Inventory.synth`, `Building.synth_food`,
/// `Market.synth_frac`, `Charity.synth_frac`, `DayRow.health`; serde
/// defaults, none written with `[sickness]` off).
/// 8: the Hit market (docs/HIT_MARKET.md H22): `Contract.deposit`,
/// `Contract.motive`, `Corp.last_wronged`, `World::hm_corp_hits`,
/// `World::wants`, `World::hm_debts`, `[hitmarket]` in the saved config
/// (serde defaults).
/// 9: the Hit market's gaps pass: `World::hm_stiffs` (the stiffed gun's
/// grace) and `[hitmarket] reach_premium`, `lawful_wait`, `queue_hours` in
/// the saved config (serde defaults).
/// 10: P10 (docs/P10_POLICY.md): the policy levers `Levers.{levy,
/// school_meals, reserve_release}`, `CityBudget.{dry_days, short_today}`
/// and `[policy]` in the saved config (serde defaults: the shipped no-net
/// market city); the saved config's retired `[economy2] wages` /
/// `no_safety_net`, `[sickness] enabled`, `[street] shanties` and
/// `[demography] school_meals` keys are ignored on load. An old save's dole
/// and public works levers load as they were saved; its levy and school
/// meals load off (an old dole-city save resumes under the market wage law).
/// 11: transit, the El (docs/TRANSIT.md): `World::transit` (closures, the day's
/// load, `rode`, the crews), `World::rides`, `DayRow.transit`,
/// `StatTable.p_dodge_caught` and `[transit]` in the saved config (serde
/// defaults, none written with the El off; a pre-transit save loads with
/// its saved config, so the El stays off for it).
/// 12: Scale S2 (docs/SCALE_20K.md): `TilePos`, `Rect`, `Building.capacity`
/// / `full_capacity`, the `guard_count` / `immigration_per_week` /
/// `sanitation_count` levers, `District.guards` / `sweepers` and
/// `FeedState.covers` widened (RON writes numbers, so an older save's
/// values read as they were); `MAX_DISTRICTS` 12 -> 32, the per-district
/// arrays (`Levers.guard_weight`, `stance_pin`, `curfew`,
/// `sanitation_weight`, `DistrictWatch`, `EconCols.d_*`) read a shorter
/// saved tuple padded (the weights with 1.0, the rest with their default);
/// the packed `DayTrace` keeps a district under 16 where it was.
/// 13: Scale S3b (docs/SCALE_20K.md): the map's grids are run-length strings
/// (`Map` saves `tiles_rle` / `zones_rle`: the whole map 1.3 MB for the
/// 3,000 x 1,875 import against ~62 MB of enum names, 61 KB for the 2k map
/// against ~550 KB), plus the map's own `districts`, `district_rle` and
/// `lines` (serde defaults). A save before 13 carries `tiles` / `zones` as
/// one enum name per tile and loads as it did (`map::MapRepr`).
/// 14: Scale S5 (docs/SCALE_20K.md § S5): `World::ambient` (the Ambient
/// Homes, the district aggregates, the donors, the counters), `[ambient]` in
/// the saved config (both skipped when empty or default: a world without
/// Ambient flats saves as before), `DayRow.ambient`, `EventKind::Materialised`.
/// 15: the Scale review fixes: `[scale] fitted` in the saved config (a
/// fitted config is not fitted again; an older save's reads `false`, so a
/// replay from it fits once more, as before), `Corp.slot` a `u16` (RON
/// writes numbers: an older save reads as it was), `World::landmark_base`
/// (the big-map landmarks' tiles while a closure left them older than the
/// map's; skipped when `None`, always on the 2,000 city), and `[transit]
/// staff_per_line` uncapped when missing (an older config keeps its crews).
/// 16: M16b phase 1, leverage (docs/M16_CONTRACTS.md § 4):
/// `World::{coercions, next_coercion, lev_tried, fake_jobs}`,
/// `DayRow.leverage`, `[leverage]` and the `persuade` keys in the saved
/// config (serde defaults: an older save loads with no coercions); the
/// appended `ContractKind::Persuade`, `Terms::Coerced`, `Origin::Court`,
/// `Crime::{Fraud, Blackmail}`, `Deed::Blackmailed` and their events.
/// 17: M16b phase 2, captives and the new kinds: `World::{captives,
/// refusals, steal_runs, feed_shamed}`, `Contract.hold`,
/// `Story.{fabricated, exposed}`, `DayRow.captive`, `[hostage]` and the
/// phase-2 kind keys in the saved config (serde defaults: an older save
/// loads with nobody held); the appended `BuildingKind::Prison`, the six
/// `ContractKind`s, `Target::{Carry, Story}`, `Origin::{Bury, Spin}`,
/// `Leverage::Hostage`, `Demand::{Sell, Release}`, `Deed::Rescued`,
/// `ActionKind::HandOver`, `RunWhy::Contract` and the four events.
/// 18: M16b phase 3, the wounded: `World::wounded`, `DayRow.wounded`,
/// `[wounded]`, `[trauma]` and the `trauma` kind keys in the saved config
/// (serde defaults: an older save loads with nobody down); the appended
/// `GoalKind::Rescue`, `ActionKind::{CarryWounded, Stabilise}`,
/// `ContractKind::Trauma`, `ContractStatus::Standing`,
/// `Origin::Subscription`, `Flow::Trauma` and the six events.
/// 19: M16b phase 4, boards, tags and attention: `Governance::Board`'s
/// `chair`, `last_vote`, `votes`; `World::{tags, next_tag, gang_scanners,
/// attention}`; `Levers.prison_permit`; `DayRow.gov`; `[governance]`,
/// `[tags]`, `[attention]` and the `tag`/`vote` kind keys in the saved
/// config (serde defaults); the appended `ContractKind::{Tag, Vote}`,
/// `Origin::Board`, `Demand::Vote`, `CoercionWhy::Vote`, the five events
/// and the eight god commands.
/// 20: the M16b review: `Wounded.{clinic, care_by}`, `[wounded] care_food`
/// (serde defaults).
/// 21: M17 phase 1, the Ambient book: `OutsideFaction.ambient`,
/// `AmbientDay.{food_wanted, food_bought, visits_wanted, visits_made,
/// paid_food, paid_visits}`, the Ambient CSV group's new columns, `[outside]`
/// in the saved config (serde defaults; skipped at its defaults), the
/// appended `Flow::AmbientSale` and `PlayerCommand::SetAmbientAppetite`.
/// 22: M17 phase 2, the parents: `OutsideFaction.parent`, `Outside.{tariff,
/// inbound_mark, outbound_mark}`, `DayRow.outside`, `[outside.parents]` in the
/// saved config (serde defaults), the appended `Flow::{Remit, ParentFund}`,
/// `EventKind::{Subsidised, ParentFailing, OutsideOrder}` and
/// `PlayerCommand::{Embargo, SetTariff}`. A save before 22 builds the
/// accounts on load (`outside::seed_parents`).
/// 23: M17 phase 3, bodies: `Parent.{held, lost, last_reinforce,
/// reinforce_left, cohort_next, abandoned_until, branch_died_at,
/// pinned_until, leaders, brothers_due, last_brothers, refounds}`,
/// `Outside.{outsiders, state_guards, mandate_until, found_streak,
/// csv_parents, today}`, the appended `OutsideOrder::{Fund, Mandate,
/// Withdraw}`, `ContractKind::Retake`, `Origin::Parent`, fourteen events and
/// `PlayerCommand::{ForceReinforce, SetOutsideOrder}`, `[outside.parents]`'s
/// phase 3 keys and the `retake` kind keys (serde defaults).
/// 24: M17 phase 4, the weapons and scrip: `Parent.{edge, data, tech,
/// scrip_out, rival}`, `Outside.{sanctions, scrip, scrip_banned}`, the
/// day's phase 4 counters, `[outside.parents]`'s phase 4 keys and the rows'
/// `data`/`tech` (serde defaults), the appended `NodeKind::Uplink`,
/// `Flow::Scrip`, six events and six god commands.
/// 25: M17 phase 4b, the succession: `Gang.{war, crowned}` (`CivilWar`),
/// the day's succession counters, `[outside.succession]` and
/// `[outside.parents]`'s `brothers_gutted_below`/`brothers_p` (serde
/// defaults), the appended `GrudgeCause::Succession`, `HuntWhy::Succession`,
/// four events and `PlayerCommand::SendBrothers`.
/// 26: M17 phase 5, Abroad, the Blackwall and the gate: `Outside.{abroad,
/// abroad_next, away, promoted, promoted_living, last_breach,
/// state_guards_cap, breach_due}`, the day's phase 5 counters,
/// `[outside.abroad]`, `[outside.blackwall]` and `[corps]`'s row-keyed
/// seeding (`fab_rows`, `noodle_first_row`, `seed_labs`, `seed_feeds`; serde
/// defaults), the appended `GrudgeCause::Abroad`, four events and
/// `PlayerCommand::{PostAbroad, BlackwallBreach, SetStateGuards, FoundParent}`.
/// 27: the M17 review: `Sanction.crew`, `Parent.founded` (serde defaults)
/// and the appended `DeathCause::Abroad`.
/// 28: Jobs & lives phase 1 (docs/JOBS_AND_LIVES.md §§ 1-2): the appended
/// `BuildingKind::{PackingPlant, FreightYard, UtilityPlant,
/// ConstructionYard, Hq}`, `Good::{Rations, Braindance, Organs}` and their
/// `ExportGood`s, `Niche::Vice`, `TradeDuty::{Fare, Corner, Preach, Sell}`,
/// four events; `Building.{under_construction_until, stock_x}`,
/// `District.dark`, `Job.until`, `Corp.office`, `World::jobs2`,
/// `DayRow.jobs2`, `[jobs2]`, the `KindCfg` columns of `[buildings.*]`, the
/// trade rows' `jobs2` flag and `[competence] admin_w`/`admin_ref` (serde
/// defaults).
/// 29: Jobs & lives phase 2, the street (§ 3): the appended
/// `GoalKind::Pitch`, `ActionKind::Vend`, `SpotKind::LabourCorner`, five
/// events; `components::Pitch` and `Jobs2State.{pitches, idle_since,
/// parcel_stops, cab_trips, yard_units}`, the street's `[jobs2]` keys, the
/// trade rows' `street` flag and the `Jobs2Cols` street columns (serde
/// defaults).
/// 30: Jobs & lives phase 3, weapons (section 4; the generic owned-item
/// store): the appended `AssetKind::Weapon`, `AssetClass::Weapon` (an
/// eleventh `asset_tax` rate, read padded), `BuildingKind::Gunsmith`,
/// `Crime::{ArmsDealing, UnlicensedArms, Smuggling}`, `Flow::Arms`, three
/// events, `PlayerCommand::FloodGuns`; `Asset.prov`,
/// `Jobs2State.arms`, `[weapons]`, `[buildings.gunsmith]`, the `weapon`
/// entries of the `[assets]` and `[tech]` per-kind tables, the
/// `sentence_days_ext` arms keys, the trade rows' `weapons` flag and the
/// `Jobs2Cols` weapons columns (serde defaults).
/// 31: Jobs & lives phase 4, vice and fashion (§ 5.1, § 9): the appended
/// `BuildingKind::{StripClub, Brothel, Boutique}`, `AssetKind::Clothing`,
/// `AssetClass::Clothing` (a twelfth `asset_tax` rate, read padded),
/// `Crime::Solicitation`, two god levers; `Needs.status`,
/// `Levers.vice`, `Jobs2State.{vice, fashion}`, `[vice]`, `[fashion]`, the
/// three `[buildings.*]` rows, the `clothing` entries of the per-kind
/// tables, the trade rows' `vice`/`fashion`/`tipped`/`min_dress` keys and the
/// `Jobs2Cols` vice and fashion columns (serde defaults).
/// 32: Jobs & lives phase 5, the chip and braindance (§ 5.2, § 5.3): the
/// appended `BuildingKind::{Dollhouse, BdStudio}`, `Slot::Behaviour`,
/// `AssetKind::Recorder`, `AssetClass::Recorder` (a thirteenth `asset_tax`
/// rate, read padded), `Crime::Trafficking`; the `Chipped` store,
/// `Jobs2State.{chip, bd}`, the `[vice]` phase-5 keys, the two
/// `[buildings.*]` rows, the `recorder` entries of the per-kind tables, the
/// trade rows' `chip`/`bd` keys and the `Jobs2Cols` phase-5 columns (serde
/// defaults).
/// 33: Jobs & lives phase 6, the gray market and faith (§ 7, § 8): the
/// appended `BuildingKind::BlackClinic`, `Crime::{Forgery, BlackMarket}`
/// (the severities above them move up), `Creed::Config`,
/// `GrudgeCause::Apostasy`; `Levers.faith`, `Jobs2State.{gray, faith}`,
/// `[gray]`, `[faith]`, `[buildings.black_clinic]`, the `recorder`-style
/// trade keys `gray`/`struck_off` and the `Jobs2Cols` phase-6 columns
/// (serde defaults).
/// 34: Jobs & lives phase 7, life stages (§ 6): the appended
/// `BuildingKind::School`, `Child.{routine, edu_days, street_days}`
/// (written only when set), `Routine`, `Levers.public_school`,
/// `Jobs2State.ages`, `[ages]`, `[buildings.school]`, the trade rows' `ages`
/// key and the `Jobs2Cols` phase-7 columns (serde defaults).
/// 35: Lives at work phase 1, the service encounter (docs/LIVES_AT_WORK.md
/// § 1): the appended `MemoryKind::Served`, `EventKind::Served`,
/// `Flow::MadamCut`, `ServeKind`; `World.regulars` (written only when
/// non-empty), `[lives]`, `[service]`, `[[serves]]` (written only on) and
/// the `Jobs2Cols` phase-1 columns (serde defaults). An off save is
/// byte-identical to version 34's.
/// 36: Lives at work phase 2, the evening and the household
/// (docs/LIVES_AT_WORK.md § 3): the appended `GoalKind::Visit`,
/// `EventKind::Visited`, `Deed::{Employed, LaidOff, Born}`,
/// `SpotKind::Home`; `Brain.empty_spots`, `Household.{newborn_until,
/// newborn_carer}`, `World.visits` (each written only when set),
/// `[household]` (written only on) and the `Jobs2Cols` phase-2 columns
/// (serde defaults). An off save is byte-identical to version 35's.
/// 37 (Lives at work phase 3, docs/LIVES_AT_WORK.md § 2.2, § 3.5):
/// `Child.edu_taught`, `PendingVisit.why`'s `Camp` arm, `World.notices_today`
/// (the day's notices, written while it holds anything), the `[service]`
/// civic keys and `[[serves]]` civic rows (written only on), the trade
/// rows' `round_near`/`tied_housing` and the `Jobs2Cols` phase-3 columns
/// (serde defaults). An off save is byte-identical to version 36's.
/// 38 (Lives at work phase 4, docs/LIVES_AT_WORK.md § 2.3): the pending cab
/// rides, parcels and the day's cab rolls on `Jobs2State` (written while
/// they hold anything), the `[service]` street keys and `[[serves]]`
/// street rows (written only on), `[stims] deal_corner` and the
/// `Jobs2Cols` phase-4 columns (serde defaults). An off save is
/// byte-identical to version 37's.
/// 39 (Lives at work phase 4b, § 2.3): `jobs2.vice.pimp_seen`,
/// `World.corner_dealers` and `World.corner_log` (each written while it
/// holds anything; empty off). An off save is byte-identical to version 38's.
/// 40 (Lives at work phase 5, § 2.4): `Story.byline`, `Deed::Decided`,
/// `VisitWhy::{Papers, Collect}`, `World.{meeting_tables, beats, bylines,
/// walk_ins}` (each written while it holds anything), the `[service]` tower
/// keys, the `[[serves]]` tower rows and the `decided` deed keys (written
/// only on) and the `Jobs2Cols` phase-5 columns (serde defaults). An off
/// save is byte-identical to version 39's.
/// 41 (Lives at work phase 6, § 4): `Job.{seniority_days, foreman}`,
/// `GangMember.{raids_led, raids_won}` (each written while non-zero), the
/// `Profession` and `Retired` stores (written while they hold anything),
/// `EventKind::Retired`, `[careers]`, `[roles]`, the trade rows'
/// `min_age`/`max_age` and the `[gangs]` lieutenants' keys (written only on)
/// and the `Jobs2Cols` phase-6 columns (serde defaults). An off save is
/// byte-identical to version 40's.
/// 42 (Lives at work phase 7, § 5.2, § 5.3): `BuildingKind::Station` (a
/// Station's held are sentenced bodies inside it, so the cells and the
/// van's list are read off `Position` and the `Sentence`: no new store),
/// `EventKind::Transferred`, the `[law]` stations' keys,
/// `[buildings.station]`, `[life] exec_home_tier`/`exec_home_spare` (written
/// only on) and the `Jobs2Cols` phase-7 columns (serde defaults). An off
/// save is byte-identical to version 41's.
/// 43 (OPEN-111): `[service] evening_staff` (written only on; the venue
/// staff's shift is the `Job::shifts` already saved). An off save is
/// byte-identical to version 42's.
/// 44 (OPEN-113): `World.service_room`, each server's encounters this hour
/// (written only when non-empty, so an off save, and an on save taken at an
/// hour's first tick before any arrival, is byte-identical to version 43's).
/// 45 (gate 8d): `Job.shift_tips` (written while non-zero), `[service]
/// move_in_rent_days` and `exec_ride` (written only on) and the `Jobs2Cols`
/// columns `exec_rides`, `exec_rides_corp` (serde defaults). An off save is
/// byte-identical to version 44's.
/// 46 (the pre-view pass phase 1, OPEN-140): `[life] housing_fix` (written
/// only on) and `World.moving_in`, the bodies moved into a Home they have
/// not walked into yet (written only when non-empty). An off save is
/// byte-identical to version 45's.
/// 47 (the pre-view pass phase 2, OPEN-137): `[shift]` (written only on),
/// `Brain.shift` (a stations shift's spots and blocks, written only while
/// set), the trade rows' `station_block_min` / `_max` (written while
/// non-zero), the `Jobs2Cols` `shift_*` columns (serde defaults) and the
/// `GoTo(Station)` / `Break` steps. An off save is byte-identical to
/// version 46's.
/// 48 (the pre-view pass phase 3, OPEN-121, OPEN-141, OPEN-145): `[shift.haul]`,
/// `[shift.counter]` and the trade rows' `shape` (written only on), the
/// `ShiftShape` `haul` and `counter`, `ShiftState.dests` / `trips` (written
/// while set), `World.counter_due` (written only when non-empty), the
/// `Unload` step and the `Jobs2Cols` `haul_*`, `counter_*`, `served_day`,
/// `served_21h`, `parcels_missed` columns (serde defaults). An off save is
/// byte-identical to version 47's.
/// 49 (the pre-view pass phase 4, OPEN-119, OPEN-120, OPEN-154): `[presence]`
/// (written only on), `Brain.sweep_route`, `Pitch.cut_paid`, `WalkIn.unpaid`,
/// `World.arms_due` and `World.fixer_owed` (each written only when set or
/// non-empty) and the `Jobs2Cols` presence columns (serde defaults). An off
/// save is byte-identical to version 48's.
/// 50: the pre-view pass phase 5 (OPEN-139, OPEN-115): `Config.legs`,
/// `World.segments`, `Jobs2State.{exec_home_rolled, passengers}`,
/// `CabRide.body`, `TransitLineCfg.in_flight` and the `Jobs2Cols` legs
/// columns (each written only when set; serde defaults). An off save is
/// byte-identical to version 49's.
/// 51: the pre-view pass phase 6 (OPEN-118, OPEN-128): `Config.leaders`,
/// `Gang.{pending_order, round_day, court_day}`, the `Court` step and the
/// `Jobs2Cols` leaders columns (each written only when set; serde
/// defaults). An off save is byte-identical to version 50's.
/// 52 (the island pass phase 0, OPEN-174): `[life] housing_fix` retired
/// with its switch (the fix always on; a version-51 save's `housing_fix:
/// true` is read and ignored). Nothing else moves: a save is version 51's
/// with that one key gone.
/// 53 (the island pass phase 1, OPEN-163): `Config.distance` (`[distance]`,
/// written only when set; serde default `None`: a version-52 save resumes
/// with the picks as they were). An off save is byte-identical to version 52's.
/// 54 (the island pass phase 2, OPEN-163, OPEN-171): `DistanceCfg.free`
/// (`[distance.free]`, written only when set; serde default `None`) and the
/// appended `LocationKey::Cot` (in a plan step only when on). An off save
/// is byte-identical to version 53's.
/// 55 (the island pass phase 3a, OPEN-163, I6): `Config.company_housing`
/// (`[company_housing]`, written only when set; serde default `None`),
/// `World.company_homes` and `World.company_lapsed` (written only when
/// non-empty) and the `Jobs2Cols` company columns (written only when
/// non-zero). An off save is byte-identical to version 54's.
/// 56 (the island pass phase 3b, OPEN-163, I6): `CompanyHousingCfg.inside`
/// and `.buyout` (`[company_housing.inside]`, `[company_housing.buyout]`,
/// written only when set; serde default `None`), `Building.within` (written
/// only when set), `World.company_notice` (written only when non-empty) and
/// the `Jobs2Cols` 3b company columns (written only when non-zero). An off
/// save is byte-identical to version 55's.
/// 57 (the island pass phase 3c, OPEN-177): `Config.mood` (`[mood.housing]`,
/// written only when set; serde default empty). An off save is
/// byte-identical to version 56's.
/// 58 (the island pass phase 4, OPEN-163, I2): `Config.commute_pressure`
/// (`[commute_pressure]`, written only when set; serde default `None`),
/// `World.commute_pressure` (written only when non-empty) and the
/// `Jobs2Cols` pressure columns (written only when non-zero). An off save
/// is byte-identical to version 57's.
/// 59 (the island pass phase 5, OPEN-165, I3): `Config.motion` (`[motion]`,
/// written only when set; serde default `None`), `World.motions` and
/// `World.aboard` (written only when non-empty: a save mid-ride resumes the
/// vehicle and its riders). An off save is byte-identical to version 58's.
/// 60 (the island pass phase 6, OPEN-165, I3): `MotionCfg.trains`
/// (`[motion.trains]`, written only when set), the trains' motions
/// (`RideKind::Train`, `Route::Line`), `TransitState.trains`, `boarding`,
/// `held` and `evac`, and `Ride.legs`/`Ride.leg` (each written only when
/// non-empty): a save mid-ride resumes the train and its riders. An off save
/// is byte-identical to version 59's.
/// 61 (the island pass phase 7, OPEN-166, I3): `MotionCfg.vans`
/// (`[motion.vans]`, written only when set), the vans' motions
/// (`RideKind::Van`, `RideKind::Foot`) and `World.van_runs` (written only
/// when non-empty): a save mid-escort or mid-transfer resumes the van and
/// its prisoners. An off save is byte-identical to version 60's.
/// 62 (the island pass phase 8, OPEN-168, I4): `Config.island_trades`
/// (`[island_trades]`, written only when set; serde default `None`) and
/// the `Jobs2Cols` `site_days` column (written only when non-zero). An off
/// save is byte-identical to version 61's.
/// 63 (the island pass phase 9, OPEN-181): `MotionCfg.walks`
/// (`[motion.walks]`, written only when set) and `ExecState::GotoTimed`'s
/// `walk` (a Coarse walk's start, written only when set): a save mid-walk
/// resumes the walk on its way. An off save is byte-identical to version
/// 62's.
/// 64 (the island pass phase 10, OPEN-185): `FviolenceCfg.hands`
/// (`[fviolence.hands]`, written only when set), `OrderRates.hands` (the
/// actor side per faction, written only when non-empty), `Hole.prebound`
/// and the `LivingCols` probe `fv_hands_capped` (each written only when
/// set): a save resumes the hands' windows and an open hole's hand. An off
/// save is byte-identical to version 63's.
/// 65 (island fix round F3, OPEN-190): `FreeDistanceCfg.stay_in`
/// (`[distance.free.stay_in]`, written only when set), `UnwindPick.stay_in`
/// and the `Jobs2Cols` `stay_in_hangouts` column (each written only when
/// set): a save mid-stay-in resumes it. An off save is byte-identical to
/// version 64's.
/// 66 (island fix round F1, OPEN-188): `Config.routes` (`[routes]`,
/// written only when set). An off save is byte-identical to version 65's.
/// 67 (the island fix round F2, OPEN-189, OPEN-191, OPEN-193):
/// `Config.beds` (`[beds]`, written only when set; serde default `None`).
/// An off save is byte-identical to version 66's.
/// 68 (the perf pass phase 10, the P9 re-import, OPEN-182): `MapBuilding.lot`
/// (a map's `lot=`, the source lot under the door, written only when set)
/// and `[life] exec_home_reach` (ISLAND_PASS I9, written only when set): an
/// island save carries its venues' lots. A save of a map without `lot=`
/// with the reach off is byte-identical to version 67's.
/// 69 (the pressure pass phase 6, a business under water): `World.underwater`
/// (written while it holds anything), `VisitWhy::{Confront, Dun}`,
/// `EventKind::{UnderWater, Loan, Confronted}`, `[underwater]` (written only
/// when set) and the `Jobs2Cols` `uw_*` columns (serde defaults).
/// Also 69 (the pressure pass phase 2, OPEN-176, merged beside phase 6): `Config.customers`
/// (`[customers]`), `ActionKind::Patch`, `Misery.patched_at`,
/// `ArmsState.quarter`, `Jobs2State.{hailed, fares_shift}` (serde defaults;
/// an old save need not load, 2026-10-26).
/// 70 (the pressure pass phase 3, protection bites): `Coercion.{missed,
/// punished}`, `VisitWhy::Punish`, `EventKind::Punished`, `[protection]`
/// (written only when set) and the `Jobs2Cols` `pr_*` columns (serde
/// defaults).
/// Also 70 (the pressure pass phase 5a, consequence, merged beside phase 3): `World.consequence`
/// (written while it holds anything), `VisitWhy::{Blackmail, Service}`,
/// `EventKind::{WitnessReacted, Blackmail, HitEscalated, Service, Cells}`,
/// `[consequence]` (written only when set) and the `Jobs2Cols` phase 5a
/// columns (serde defaults).
/// 71 (the pressure pass phase 5b, the gang half): `World.crew` (written
/// while it holds anything), `VisitWhy::{Crew, Reckoning}`,
/// `EventKind::Crew`, `[crew]` (written only when set) and the `Jobs2Cols`
/// `crew_*` columns (serde defaults).
/// 72 (pressure pass round 2, R1, the fare's ladder): `World.fares`
/// (written while it holds anything), `EventKind::Fare`, `[fares]` (written
/// only when set) and the `Jobs2Cols` `fare_*` columns (serde defaults).
/// 73 (the pressure pass round 2, R4): `World.notice_fold` (written while it
/// holds anything) and `[noise]` (serde defaults).
/// 74 (pressure pass round 2, R3, the wounded and the sick): `World.care`
/// (written while it holds anything), `Wounded.called`, `[care]` (written
/// only when set) and the `Jobs2Cols` `care_*` columns (serde defaults).
/// 75 (pressure pass round 2, R5, the gang verbs): `crew::State.stood_down`,
/// the `[crew]` R5 keys and the `Jobs2Cols` `crew_stood_down`,
/// `crew_tryout_refused` columns (serde defaults).
/// 76 (pressure pass round 2, R6a, the trades' verbs I): `ViceState.{pimp_visits,
/// bookings, barred}` (written while they hold anything) and `[trade_verbs]`
/// (written only when set).
pub const SAVE_VERSION: u8 = 76;

/// Compact RON of the whole world. Same world state ⇒ same bytes.
pub fn to_ron(world: &World) -> String {
    ron::to_string(world).expect("World is always serialisable")
}

pub fn from_ron(text: &str) -> Result<World, ron::error::SpannedError> {
    // J32: a `Trade("key")` resolves against the trades of the config this
    // save carries (read before any Job): none until that config is read.
    crate::components::TradeId::set_keys(Vec::new());
    let mut world: World = ron::from_str(text)?;
    // Review finding 10: the landmarks the saved world searched with.
    let base = world.landmark_base.as_ref().map(|b| b.0.clone()).filter(|t| t.len() == world.map.tiles().len());
    world.flow_fields.set_landmark_base(base);
    world.resize_stores();
    world.rebuild_indices();
    // M12 D1: the district grid and adjacency are not saved.
    crate::systems::districts::rebuild(&mut world);
    // Transit: the derived network (not saved).
    crate::systems::transit::build(&mut world);
    // M17 phase 2: a save from before the parents builds their accounts
    // (minted, so the identity carries over; nothing with `[outside]` off).
    if world.save_version < 22 {
        crate::systems::outside::seed_parents(&mut world);
    }
    // M17 phase 4: a parent from before the weapons gets its edge, Data and tech ceiling.
    if world.save_version < 24 {
        crate::systems::parents::migrate_v24(&mut world);
    }
    world.save_version = SAVE_VERSION;
    world.reload_names();
    Ok(world)
}

/// `saves/<seed>-<tick>.ron`.
pub fn save_path(dir: &Path, seed: u64, tick: u64) -> PathBuf {
    dir.join(format!("{seed}-{tick}.ron"))
}

pub fn save_to_file(world: &World, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, to_ron(world))
}

pub fn load_from_file(path: &Path) -> Result<World, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    from_ron(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The save with the highest tick for `seed` in `dir`, if any.
pub fn newest_save(dir: &Path, seed: u64) -> Option<PathBuf> {
    let prefix = format!("{seed}-");
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(rest) = name.strip_prefix(&prefix).and_then(|r| r.strip_suffix(".ron")) else { continue };
        let Ok(tick) = rest.parse::<u64>() else { continue };
        if best.as_ref().is_none_or(|(t, _)| tick > *t) {
            best = Some((tick, entry.path()));
        }
    }
    best.map(|(_, p)| p)
}
