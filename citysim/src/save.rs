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
pub const SAVE_VERSION: u8 = 32;

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
