//! Daily statistics: counters that systems bump during the day and
//! snapshots taken at the end of it. `--report` prints one row per day.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::components::{CorpOrder, MAX_DISTRICTS};
use crate::time::Season;

/// Days of history kept for the city panel's sparklines.
pub const STATS_HISTORY_CAP: usize = 120;

pub const CSV_HEADER: &str = "day,season,population,employed,homeless,jailed,gang_members,food_market,food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,mean_mood,goal_changes_per_agent,holes_opened,holes_open,holes_bound,holes_unknown,deaths_violence_offscreen,tier_full,tier_coarse,tier_stat,evictions,rent_paid,rent_short,housed,flow_food,flow_drink,flow_wages,flow_rent,flow_upkeep,flow_wholesale,flow_overflow,flow_restock,flow_contract,flow_tax,flow_dole,flow_other,wallets,wallet_gini,wallet_top10,corp1_treasury,corp1_order,corp2_treasury,corp2_order,corp3_treasury,corp3_order,corp4_treasury,corp4_order,corp5_treasury,corp5_order,corp6_treasury,corp6_order,corp7_treasury,corp7_order,corp8_treasury,corp8_order,corp9_treasury,corp9_order,acquisitions,bankruptcies,monopolies,foundings,incorporations,strikes,unrest_corp,unrest_street,unrest_dreg,class_corp,class_street,class_dreg,happiness_street,d1_coverage,d1_control,d1_litter,d1_unrest,d1_crime,d1_guards,d2_coverage,d2_control,d2_litter,d2_unrest,d2_crime,d2_guards,d3_coverage,d3_control,d3_litter,d3_unrest,d3_crime,d3_guards,d4_coverage,d4_control,d4_litter,d4_unrest,d4_crime,d4_guards,d5_coverage,d5_control,d5_litter,d5_unrest,d5_crime,d5_guards,d6_coverage,d6_control,d6_litter,d6_unrest,d6_crime,d6_guards,d7_coverage,d7_control,d7_litter,d7_unrest,d7_crime,d7_guards,d8_coverage,d8_control,d8_litter,d8_unrest,d8_crime,d8_guards,hotel_nights,squatters,derelicts,vagrancy,riots,crossfire,gangs,vehicles_moto,vehicles_car,vehicles_truck,vehicles_flyer,truck_hauls,walk_hauls,commute_tpt_walk,commute_tpt_drive,chrome_installs,chrome_agents,mean_sanity,episodes,hooked,stims_dealt,stims_legal,dealing_reports,repos,impounds,crashes,crash_deaths,vehicle_thefts,chops,abductions,stripped,robots,flow_asset,flow_asset_upkeep,flow_finance,flow_import,flow_stims,flow_parts,flow_treatment,overdoses,harvests,stripped_window,gang_income,gang_income_dealing,episodes_by_law,treatments,detoxes,\
nodes,labs,decks,runs,runs_ok,data_made,data_stolen,data_wiped,data_sold,ledger_hacks,doors_hacked,traced,fried,flatlined,hack_arrests,ice_mean_corp,ice_spend,\
runs_bounced,runs_captured,runs_dumped,hack_arrests_chair,robots_turned,blinded,cameras,sightings,ice_raised,ice_lowered,tech_gained,tech_lost,research_spent,data_held,\
corp1_tier_chrome,corp1_tier_deck,corp1_tier_industry,corp1_data,corp2_tier_chrome,corp2_tier_deck,corp2_tier_industry,corp2_data,corp3_tier_chrome,corp3_tier_deck,corp3_tier_industry,corp3_data,corp4_tier_chrome,corp4_tier_deck,corp4_tier_industry,corp4_data,corp5_tier_chrome,corp5_tier_deck,corp5_tier_industry,corp5_data,corp6_tier_chrome,corp6_tier_deck,corp6_tier_industry,corp6_data,corp7_tier_chrome,corp7_tier_deck,corp7_tier_industry,corp7_data,corp8_tier_chrome,corp8_tier_deck,corp8_tier_industry,corp8_data,corp9_tier_chrome,corp9_tier_deck,corp9_tier_industry,corp9_data,\
flow_data,flow_hack,flow_ice_upkeep,flow_research,flow_terminal,\
rumours_heard,second_hand_share,known_by_killers_median,distorted,grudges,grudges_inherited,hunts,hunts_active,avenged,revenge_kills,chain_max,vendettas_open,stories,planted,buried,poached,talent_lost,skill_rare_share,extort_success,rep_flips,rumour_hops_max,pool_reach,contradicted,silenced,hunts_failed,hunts_abandoned,guard_body,expelled,contracts_lost_honour,extort_tries,\
g1_dread,g1_heat,g2_dread,g2_heat,g3_dread,g3_heat,g4_dread,g4_heat,\
c1_honour,c1_standing,c1_competence,c2_honour,c2_standing,c2_competence,c3_honour,c3_standing,c3_competence,c4_honour,c4_standing,c4_competence,c5_honour,c5_standing,c5_competence,c6_honour,c6_standing,c6_competence,c7_honour,c7_standing,c7_competence,c8_honour,c8_standing,c8_competence,c9_honour,c9_standing,c9_competence,law_competence,flow_ads,flow_plant,\
flow_leisure,flow_gamble,flow_gamble_win,flow_tribute,flow_export,flow_public_works,wage_dole_ratio,employed_share,venues_club,venues_arcade,venues_noodle_bar,venues_fight_pit,venues_den,venues_lounge,visits_club,visits_arcade,visits_noodle_bar,visits_fight_pit,visits_den,visits_lounge,fab_parts,scrap_parts,parts_imported,works_jobs,upkeep_mult,outside_inbound,outside_minted,fun_mean,fun_satisfied_share,hangouts,hangout_contacts_mean,fronts,collected,preached,d1_street_density,d2_street_density,d3_street_density,d4_street_density,d5_street_density,d6_street_density,d7_street_density,d8_street_density,fv_killed,fv_assaulted,fv_robbed,fv_abducted,fv_bound,fv_unknown,fv_capped,fv_bound_wrong,kill_rate_body,kill_rate_stat,kill_rate_body_civ,kill_rate_stat_civ,\
tier_held,tier_held_body,held_fed,gang_bodies,gang_stat,stat_extorts,stat_claims,stat_deals,aborts,aborts_scavenge,aborts_sleep,aborts_checkin,aborts_seat,rough_sleeps,scavenge_dry,flow_street_dice,\
contracts_open,contracts_posted,contracts_fulfilled,contracts_failed,contracts_expired,contracts_cancelled,reneged,k_hit_posted,k_hit_done,k_beat_posted,k_beat_done,k_guard_posted,k_guard_done,k_locate_posted,k_locate_done,hits_done,hits_squad,hits_solo_weak,strikes_declined_pol,sold_out,contract_murders,contract_cleared,contract_holes,contract_hole_wrong,accessory,interrogations,interrogations_won,bounties_paid,guards_on_take,fixer_runs,regulars,live_parties,live_queued,escrow_held,escrow_leak,escrow_stuck,f0_heat,f1_heat,f2_heat,f3_heat,f0_income,f1_income,f2_income,f3_income,flow_escrow,flow_payout,flow_fixer_cut,contracts_refused_full,flow_import_out,flow_customs,flow_inputs,flow_property,flow_donate,trade_balance,trade_balance_30,appetite_food,appetite_parts,appetite_data,bid_food,ask_food,food_imported,food_exported,wage_mult_mean,wage_gross_mean,vacancies_open,laid_off,pop_inflow_wages,pop_inflow_other,tax_rate,mission_meals,mission_cots,mission_purse,flow_migrant_in,flow_migrant_out,flow_fence,flow_alms,flow_camp_food,outside_outbound,world_treasury,coin_identity,parts_exported,data_exported,export_paid,grow_world,city_owned_buildings,recycler_till,children_taken,camp_children,camp_output,camp_unfed,camp_released,camp_scandals,d1_meals,d1_starving,d1_dregs,d2_meals,d2_starving,d2_dregs,d3_meals,d3_starving,d3_dregs,d4_meals,d4_starving,d4_dregs,d5_meals,d5_starving,d5_dregs,d6_meals,d6_starving,d6_dregs,d7_meals,d7_starving,d7_dregs,d8_meals,d8_starving,d8_dregs,civic_payroll,flow_fines,guard_count,sanitation_count,migrants_pull,migrants_offer,trades_employed,super_rounds,tax_credit,meals_real,meals_synth,meals_dive,meals_alms,malnourished,toxic,mean_malnutrition,mean_toxicity,sick_days,cancer_new,cancer_diagnosed,cancer_cured,deaths_disease,bin_meals_in,bin_contests,bin_shakedowns,hm_posted_revenge,hm_posted_rival,hm_posted_culprit,hm_posted_silence,hm_posted_debt,hm_posted_protect,hm_wants,hm_unaffordable,hm_refused,hm_pooled,hm_price_sum,hm_deposit_sum,hm_hits_done,hm_beats_done,hm_stiffed,hm_recanted,hm_cold,hm_commissions,coercions_active,lev_threat_tries,lev_threat_ok,lev_secret_tries,lev_secret_ok,lev_debt_tries,lev_debt_ok,coercions_broken,protection_made,protection_paid,protection_coins,protection_active,blackmail_made,blackmail_active,secrets_lapsed,silenced_threat,coerced_jobs,coerced_done,poach_threats,parleys,parley_fails,tribute_active,fake_jobs,frauds_filed,k_persuade_posted,k_persuade_done,flow_coerced,captives_held,prison_pop,prisons,hostages_taken,hostages_freed,ransomed,rescued_captives,escaped,hostages_killed,harvest_handovers,captives_unfed,lev_hostage_tries,lev_hostage_ok,k_abduct_posted,k_abduct_done,k_rescue_posted,k_rescue_done,k_escort_posted,k_escort_done,k_steal_posted,k_steal_done,k_deliver_posted,k_deliver_done,k_spin_posted,k_spin_done,fabricated,exposed,steal_runs,flow_ransom,downed,wounds_capped,bled_out,finished_off,stabilised,recovered,rescued_kin,rescued_gang,rescued_colleague,rescued_guard,trauma_calls,trauma_saves,down_now,trauma_subs,trauma_subs_new,trauma_lapsed,stat_wounds,flow_trauma,votes,votes_swung,chairs_fallen,boards,campaign_moves,board_persuades,k_vote_posted,k_vote_done,k_tag_posted,k_tag_done,tags_planted,tags_live,tags_found,scans,patrols,patrol_finds,scanners_bought,distracted,alert_law,alert_min,rides_full,rides_coarse,rides_stat,fares,fare_dodges,refused_no_route,refused_fare,refused_slow,refused_danger,ride_abandoned,peak_stop_load,transit_power,el_staff,commute_tpt_ride,ticks_per_sec";

/// D38: corp CSV slots (seeding order). M13 D17: 9 (the Tech corp from phase 2).
/// Scale S2: the floor; a config seeding more corps prints one slot per row
/// ([`csv_header`]).
pub const CORP_SLOTS: usize = 9;

/// M15 W43: gang CSV slots (`World::gang_index` order).
pub const GANG_SLOTS: usize = 4;

/// M12 D45: district CSV slots (`[districts]` row order). Scale S2: the
/// floor; a config with more districts prints one slot per district
/// ([`csv_header`]).
pub const DISTRICT_SLOTS: usize = 8;

/// Scale S2: the header for `corps` corp slots and `districts` district
/// slots: [`CSV_HEADER`] itself while both fit its [`CORP_SLOTS`] and
/// [`DISTRICT_SLOTS`] (the 2k city's CSV is unchanged), else every slot
/// group grown past its last slot (`corp10_treasury,corp10_order` after
/// `corp9_order`, `d9_coverage..d9_guards` after `d8_guards`, and so on).
/// The rows match it: a row's slot vectors are as long as the run's slots
/// (`systems::stats::csv_slots`).
pub fn csv_header(corps: usize, districts: usize) -> String {
    if corps <= CORP_SLOTS && districts <= DISTRICT_SLOTS {
        return CSV_HEADER.to_string();
    }
    // `corp9_order` -> ("corp", 9, "order"); `d8_meals` -> ("d", 8, "meals").
    fn slot(col: &str) -> Option<(&str, usize, &str)> {
        let prefix = ["corp", "c", "d"].into_iter().find(|p| col.starts_with(p))?;
        let rest = &col[prefix.len()..];
        let digits = rest.find('_')?;
        let n = rest[..digits].parse().ok()?;
        Some((prefix, n, &rest[digits + 1..]))
    }
    let cols: Vec<&str> = CSV_HEADER.split(',').collect();
    let mut out: Vec<String> = Vec::with_capacity(cols.len());
    let mut block: Vec<&str> = Vec::new();
    for (i, &col) in cols.iter().enumerate() {
        out.push(col.to_string());
        let Some((prefix, n, suffix)) = slot(col) else { continue };
        let (last, want) = if prefix == "d" { (DISTRICT_SLOTS, districts) } else { (CORP_SLOTS, corps) };
        if n != last {
            continue;
        }
        block.push(suffix);
        let next = cols.get(i + 1).and_then(|c| slot(c));
        if next.is_some_and(|(p, m, _)| p == prefix && m == n) {
            continue;
        }
        for k in last + 1..=want {
            out.extend(block.iter().map(|s| format!("{prefix}{k}_{s}")));
        }
        block.clear();
    }
    out.join(",")
}

/// M12 D45: one district's CSV numbers: coverage, control code, litter,
/// unrest, crime rate, guards.
pub type DistrictCols = (f32, u8, f32, f32, f32, u16);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DayRow {
    pub day: u64,
    pub season: Season,
    // --- snapshots, taken at the end of the day ---
    pub population: u32,
    pub employed: u32,
    pub homeless: u32,
    pub jailed: u32,
    pub gang_members: u32,
    pub food_market: u32,
    pub food_warehouse: u32,
    pub food_pantry: u32,
    pub price: i64,
    pub treasury: i64,
    pub mean_hunger: f32,
    pub mean_mood: f32,
    // --- counters, bumped by systems during the day ---
    pub thefts: u32,
    pub arrests: u32,
    pub deaths_starvation: u32,
    pub deaths_old_age: u32,
    pub deaths_violence: u32,
    pub births: u32,
    pub immigrants: u32,
    pub emigrants: u32,
    pub burials: u32,
    /// Goals displaced mid-plan by a different winner (flapping). Raw count;
    /// divided by population into `goal_changes_per_agent` at day end.
    pub goal_changes: u32,
    pub goal_changes_per_agent: f32,
    /// M10 counters: holes opened, bound to an actor, closed as Unknown, and
    /// Statistical agents killed off screen.
    #[serde(default)]
    pub holes_opened: u32,
    #[serde(default)]
    pub holes_bound: u32,
    #[serde(default)]
    pub holes_unknown: u32,
    #[serde(default)]
    pub deaths_violence_offscreen: u32,
    /// M10 snapshots: open holes and the tier sizes at day end.
    #[serde(default)]
    pub holes_open: u32,
    #[serde(default)]
    pub tier_full: u32,
    #[serde(default)]
    pub tier_coarse: u32,
    #[serde(default)]
    pub tier_stat: u32,
    /// M11 counters: evicted adults, rent collected, adults short on a due
    /// rent, adults re-housed.
    #[serde(default)]
    pub evictions: u32,
    #[serde(default)]
    pub rent_paid: i64,
    #[serde(default)]
    pub rent_short: u32,
    #[serde(default)]
    pub housed: u32,
    /// M11 D3 ledger: gross coins moved per flow today (`ownership::pay` /
    /// `charge`). `flow_tax` is tax moved into the Treasury from other
    /// payers (owner revenue and non-city wages); a city wage's withheld tax
    /// never leaves the Treasury and is not counted. `flow_dole` is the dole
    /// paid; `flow_other` is sales, subsidies, bribes, jail food, SellFood.
    #[serde(default)]
    pub flow_food: i64,
    #[serde(default)]
    pub flow_drink: i64,
    #[serde(default)]
    pub flow_wages: i64,
    #[serde(default)]
    pub flow_rent: i64,
    #[serde(default)]
    pub flow_upkeep: i64,
    #[serde(default)]
    pub flow_wholesale: i64,
    /// Of `flow_wholesale`: the city buying a non-city Farm's overflow into
    /// the Reserve, and non-city Markets buying their restock from it (D5).
    #[serde(default)]
    pub flow_overflow: i64,
    #[serde(default)]
    pub flow_restock: i64,
    #[serde(default)]
    pub flow_contract: i64,
    #[serde(default)]
    pub flow_tax: i64,
    #[serde(default)]
    pub flow_dole: i64,
    #[serde(default)]
    pub flow_other: i64,
    /// M11 snapshots: coins in living adults' wallets, their Gini, and the
    /// richest tenth's share (VISION: an unequal city from day one).
    #[serde(default)]
    pub wallets: i64,
    #[serde(default)]
    pub wallet_gini: f32,
    #[serde(default)]
    pub wallet_top10: f32,
    /// M11 D38 snapshot: each seeded corp's treasury and order by slot
    /// (`None` once dissolved).
    #[serde(default)]
    pub corps: Vec<Option<(i64, CorpOrder)>>,
    /// M11 phase 3 counters: buildings that changed hands by sale (hostile
    /// acquisitions and bankruptcy sales), corps gone bankrupt; and a
    /// snapshot: corp-niche pairs at monopoly at day end.
    #[serde(default)]
    pub acquisitions: u32,
    #[serde(default)]
    pub bankruptcies: u32,
    #[serde(default)]
    pub monopolies: u32,
    /// M11 phase 4 counters: NPC foundings (`Register`; a corp's `Grow` build
    /// is not counted), incorporations, strikes.
    #[serde(default)]
    pub foundings: u32,
    #[serde(default)]
    pub incorporations: u32,
    #[serde(default)]
    pub strikes: u32,
    /// M11 phase 4 snapshots from `World::classes` (computed at midnight):
    /// unrest per class, class sizes, Street happiness.
    #[serde(default)]
    pub unrest_corp: f32,
    #[serde(default)]
    pub unrest_street: f32,
    #[serde(default)]
    pub unrest_dreg: f32,
    #[serde(default)]
    pub class_corp: u32,
    #[serde(default)]
    pub class_street: u32,
    #[serde(default)]
    pub class_dreg: u32,
    #[serde(default)]
    pub happiness_street: f32,
    /// M12 D45 snapshot: per district slot (`DISTRICT_SLOTS`).
    #[serde(default)]
    pub districts: Vec<DistrictCols>,
    /// M12 D45: squatters, derelict buildings and gangs are day-end
    /// snapshots; hotel nights, Vagrancy fines and sentences, riots and
    /// crossfire hits are daily counters (zero until their phase). The Dreg
    /// adults are `class_dreg` (M12 review: the `dregs` column repeated it).
    #[serde(default)]
    pub hotel_nights: u32,
    #[serde(default)]
    pub squatters: u32,
    #[serde(default)]
    pub derelicts: u32,
    #[serde(default)]
    pub vagrancy: u32,
    #[serde(default)]
    pub riots: u32,
    #[serde(default)]
    pub crossfire: u32,
    #[serde(default)]
    pub gangs: u32,
    /// M12 D38: raids and breakouts that reached a door under full cover
    /// (the stance turned mid-march); not a CSV column (the gate reads it).
    #[serde(default)]
    pub raids_into_cover: u32,
    /// M13 D49: vehicles by kind, chromed agents, the mean sanity of adults
    /// with a Body, hooked adults, the legal-stims lever and posted robots
    /// are day-end snapshots; the rest are daily counters (zero until their
    /// phase). `commute_tpt_*` are ticks per Manhattan tile of the day's
    /// arrivals at work, walked and driven.
    #[serde(default)]
    pub vehicles_moto: u32,
    #[serde(default)]
    pub vehicles_car: u32,
    #[serde(default)]
    pub vehicles_truck: u32,
    #[serde(default)]
    pub vehicles_flyer: u32,
    #[serde(default)]
    pub truck_hauls: u32,
    #[serde(default)]
    pub walk_hauls: u32,
    #[serde(default)]
    pub commute_tpt_walk: f32,
    #[serde(default)]
    pub commute_tpt_drive: f32,
    #[serde(default)]
    pub chrome_installs: u32,
    #[serde(default)]
    pub chrome_agents: u32,
    #[serde(default)]
    pub mean_sanity: f32,
    #[serde(default)]
    pub episodes: u32,
    #[serde(default)]
    pub hooked: u32,
    #[serde(default)]
    pub stims_dealt: u32,
    #[serde(default)]
    pub stims_legal: u32,
    #[serde(default)]
    pub dealing_reports: u32,
    #[serde(default)]
    pub repos: u32,
    #[serde(default)]
    pub impounds: u32,
    #[serde(default)]
    pub crashes: u32,
    #[serde(default)]
    pub crash_deaths: u32,
    #[serde(default)]
    pub vehicle_thefts: u32,
    #[serde(default)]
    pub chops: u32,
    #[serde(default)]
    pub abductions: u32,
    #[serde(default)]
    pub stripped: u32,
    #[serde(default)]
    pub robots: u32,
    #[serde(default)]
    pub flow_asset: i64,
    #[serde(default)]
    pub flow_asset_upkeep: i64,
    #[serde(default)]
    pub flow_finance: i64,
    #[serde(default)]
    pub flow_import: i64,
    #[serde(default)]
    pub flow_stims: i64,
    #[serde(default)]
    pub flow_parts: i64,
    #[serde(default)]
    pub flow_treatment: i64,
    #[serde(default)]
    pub overdoses: u32,
    #[serde(default)]
    pub harvests: u32,
    #[serde(default)]
    pub stripped_window: u32,
    #[serde(default)]
    pub gang_income: i64,
    #[serde(default)]
    pub gang_income_dealing: i64,
    #[serde(default)]
    pub episodes_by_law: u32,
    #[serde(default)]
    pub treatments: u32,
    #[serde(default)]
    pub detoxes: u32,
    /// M14 V43: the Virt columns (zero with the plane off).
    #[serde(default)]
    pub virt: VirtCols,
    /// M15 W43: the word's columns (zero with `[gossip]` off).
    #[serde(default)]
    pub word: WordCols,
    /// L2 (plan L34): the living city's columns (zero with `[living]` off,
    /// but for `parts_imported`).
    #[serde(default)]
    pub living: LivingCols,
    /// L2 phase 3 (L34): the LOD budget's and the churn's columns.
    #[serde(default, skip_serializing_if = "BudgetCols::is_zero")]
    pub budget: BudgetCols,
    /// M16a (plan C38): the contract records' columns (zero with
    /// `[contracts]` off; created whole in phase 1, zeros until their phase).
    #[serde(default, skip_serializing_if = "ContractCols::is_zero")]
    pub contract: ContractCols,
    /// The Real economy (plan E45): one group after M16a's, created whole
    /// in phase 1 (zeros until their phase).
    #[serde(default, skip_serializing_if = "EconCols::is_zero")]
    pub econ: EconCols,
    /// Jobs v2 (plan J34): the jobs pass's group, after the Real economy's
    /// and before `ticks_per_sec`; later phases append to it.
    #[serde(default, skip_serializing_if = "JobsCols::is_zero")]
    pub jobs: JobsCols,
    /// Hunger as sickness (docs/HUNGER_SICKNESS.md § 9): after the jobs
    /// group, before `ticks_per_sec` (zeros with `[sickness]` off).
    #[serde(default, skip_serializing_if = "HealthCols::is_zero")]
    pub health: HealthCols,
    /// The Hit market (docs/HIT_MARKET.md § 7): after the jobs group,
    /// before `ticks_per_sec`.
    #[serde(default, skip_serializing_if = "HitCols::is_zero")]
    pub hit: HitCols,
    /// M16b § 4 (phase 1): leverage's group, after the Hit market's.
    #[serde(default, skip_serializing_if = "LeverageCols::is_zero")]
    pub leverage: LeverageCols,
    /// M16b phase 2: captives and the new kinds, after leverage's.
    #[serde(default, skip_serializing_if = "CaptiveCols::is_zero")]
    pub captive: CaptiveCols,
    /// M16b phase 3: the wounded, after the captives'.
    #[serde(default, skip_serializing_if = "WoundedCols::is_zero")]
    pub wounded: WoundedCols,
    /// M16b phase 4: boards, tags and attention, after the wounded's.
    #[serde(default, skip_serializing_if = "GovCols::is_zero")]
    pub gov: GovCols,
    /// Transit (docs/TRANSIT.md § 4): the El's group, after the hit group
    /// and before `ticks_per_sec` (zero, and not saved, with transit off).
    #[serde(default, skip_serializing_if = "TransitCols::is_zero")]
    pub transit: TransitCols,
    /// Scale S5 (docs/SCALE_20K.md § S5): the Ambient tier's group, after the
    /// transit group and before `ticks_per_sec`, only on a world with an
    /// Ambient tier (`None` and no columns otherwise: the 2k CSV is unchanged;
    /// the header from `systems::stats::csv_header`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<AmbientCols>,
    /// M17 phase 2: the parents' group, after the Ambient group and before
    /// `ticks_per_sec`, only with the parents on (`None` and no columns
    /// otherwise; the header from `systems::stats::csv_header`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outside: Option<OutsideCols>,
    /// Jobs & lives (docs/JOBS_AND_LIVES.md § 12): the milestone's group,
    /// after the parents' and before `ticks_per_sec`, only with `[jobs2]
    /// enabled` (`None` and no columns otherwise: the off CSV is unchanged;
    /// the header from `systems::stats::csv_header`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs2: Option<Jobs2Cols>,
    /// Filled in by the runner (the library has no clock).
    pub ticks_per_sec: f32,
}

/// Jobs & lives phase 1 (§ 2, § 12's printed bands): the industries' flows
/// and units, the builds, the outages, the civic three and the Hq, and
/// jobs by `TradeModel` and by new kind (snapshots at the day's end).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Jobs2Cols {
    /// `Flow::Handling` paid to Freight Yards (fees on crossings and hauls).
    pub flow_handling: i64,
    /// Handling fees that went to the World (no staffed Yard in reach).
    pub handling_leak: i64,
    /// `Flow::Power` paid to Utility Plants.
    pub flow_power: i64,
    /// The plants' fuel share to the World (`Flow::Inputs`).
    pub power_fuel: i64,
    /// `Flow::Build` paid to Construction Yards.
    pub flow_build: i64,
    /// Food units packed and Rations made, sold to the World and paid.
    pub food_packed: u32,
    pub rations_made: u32,
    pub rations_sold: u32,
    pub rations_paid: i64,
    /// Rations a Loader walked out with.
    pub pilfered: u32,
    /// Builds started and finished at Yards today; sites open at the end.
    pub builds_started: u32,
    pub builds_done: u32,
    pub sites_open: u32,
    /// Districts dark at the day's end, and outages begun today.
    pub dark_districts: u32,
    pub outages: u32,
    /// The civic three's headcounts (Job holders) and the allocation's targets.
    pub jailers: u32,
    pub clerks: u32,
    pub depot_hands: u32,
    pub civic3_target: u32,
    /// Office workers and standing Hqs.
    pub office_workers: u32,
    pub hqs: u32,
    /// Trade Job holders by `TradeModel` (row order of the enum).
    pub emp_produce: u32,
    pub emp_service: u32,
    pub emp_handling: u32,
    pub emp_contract: u32,
    pub emp_civic: u32,
    pub emp_overhead: u32,
    pub emp_resale: u32,
    pub emp_day: u32,
    /// Job holders at each new kind (every role).
    pub emp_packing: u32,
    pub emp_freight: u32,
    pub emp_utility: u32,
    pub emp_construction: u32,
    pub emp_hq: u32,
    /// Standing buildings of each new kind.
    pub n_packing: u32,
    pub n_freight: u32,
    pub n_utility: u32,
    pub n_construction: u32,
    // --- Phase 2, the street (snapshots: the Pitches open by kind and the
    // day labourers; the rest the day's counts and coins).
    pub pitches: u32,
    pub pitches_vendor: u32,
    pub pitches_rickshaw: u32,
    pub pitches_tinker: u32,
    pub pitch_opened: u32,
    pub pitch_closed: u32,
    pub street_cut: i64,
    pub swept: u32,
    pub vendor_meals: u32,
    pub vendor_take: i64,
    pub day_posts_offered: u32,
    pub day_posts_filled: u32,
    pub day_posts_ended: u32,
    pub corner_queue: u32,
    pub day_labourers: u32,
    pub parcels: u32,
    pub parcel_fees: i64,
    pub cab_trips: u32,
    pub cab_fares: i64,
    pub rickshaw_trips: u32,
    pub rickshaw_fares: i64,
    pub tinker_repairs: u32,
    pub tinker_take: i64,
    pub tinker_botched: u32,
    /// Utility Plants the city took over from a broke owner (dark too long).
    pub plants_seized: u32,
    // --- Phase 3, weapons.
    /// Weapon entities alive at the day's end; agents carrying one; in the gangs' armouries, the Precinct's and the Gunsmiths' stock.
    pub weapons: u32,
    pub carriers: u32,
    pub armoury: u32,
    pub precinct_arms: u32,
    pub gunsmith_stock: u32,
    /// Licence holders on the Clerk's register, and adults who want a weapon.
    pub licences: u32,
    pub weapon_wants: u32,
    /// The day's counts: made, sold at a Gunsmith, sold by a dealer, issued at a muster, seized by the law.
    pub weapons_made: u32,
    pub sold_legal: u32,
    pub sold_dealer: u32,
    pub issued: u32,
    pub seized: u32,
    /// Crates landed and seized at customs.
    pub crates_landed: u32,
    pub crates_seized: u32,
    /// Fights and the deaths in them; those with either side armed, and
    /// the deaths in those.
    pub fights: u32,
    pub fight_deaths: u32,
    pub armed_fights: u32,
    pub armed_deaths: u32,
    /// `UnlicensedArms` reports filed at sightings; licences sold.
    pub ua_reports: u32,
    pub licences_sold: u32,
    /// Petty item thefts (a carrier's pocket, a stall's till, a Home), and items fenced.
    pub pickpockets: u32,
    pub stall_thefts: u32,
    pub home_thefts: u32,
    pub fenced: u32,
    /// The pressure pass phase 1 (OPEN-178, `systems::misery`): shifts
    /// skipped, absentees dismissed, quits, brawls, gang joins and
    /// conversions that misery opened.
    pub misery_skips: u32,
    pub misery_fired: u32,
    pub misery_quits: u32,
    pub brawls: u32,
    pub misery_joins: u32,
    pub misery_converts: u32,
    /// The pressure pass phase 6 (`systems::underwater`): owner-days under
    /// water; the owners' choices (hang on, shorter hours, layoffs, loans,
    /// sales to a corp or a gang, closures); missed installments, the
    /// collectors' beatings, businesses seized, loans repaid; shut
    /// businesses reopened by a new owner, businesses afloat again; unpaid
    /// walkouts; the stiffed workers' confrontations, paid, fights, reports,
    /// garnishments; a front's staff joining, its laundered coins, rival hits.
    pub uw_short: u32,
    pub uw_hold: u32,
    pub uw_hours: u32,
    pub uw_layoffs: u32,
    pub uw_loans: u32,
    pub uw_missed: u32,
    pub uw_beatings: u32,
    pub uw_seized: u32,
    pub uw_repaid: u32,
    pub uw_sold_corp: u32,
    pub uw_sold_gang: u32,
    pub uw_closed: u32,
    pub uw_reopened: u32,
    pub uw_afloat: u32,
    pub uw_walkouts: u32,
    pub uw_confronts: u32,
    pub uw_paid_up: u32,
    pub uw_confront_fights: u32,
    pub uw_reports: u32,
    pub uw_garnished: u32,
    pub uw_front_joins: u32,
    pub uw_front_hits: u32,
    pub uw_laundered: i64,
    /// The pressure pass phase 2 (OPEN-176, `systems::customers`): door
    /// scenes, lonely clients' visits, hailed cabs, fear's gun wants, gang
    /// buys from another gang's dealer, patched wounds (a Clinic's and a
    /// back room's patients), back rooms opened, vendor meals
    /// sold to bodies.
    pub door_scenes: u32,
    pub lonely_visits: u32,
    pub cab_hails: u32,
    pub fear_wants: u32,
    pub gang_arms_buys: u32,
    pub patched: u32,
    pub black_patients: u32,
    pub back_rooms: u32,
    pub vendor_bodies: u32,
    /// The pressure pass phase 5a (`systems::consequence`): the witnesses'
    /// reactions (report, flee, gossip, blackmail; the culprit saw who saw
    /// him); blackmail demands made, paid (and the coins), silenced,
    /// reported; unfilled hits reposted higher, done by the buyer himself,
    /// let go; the cells (a reporter's grudge, a changed release, jail
    /// absences and firings, fights, recruits, a Fixer's name, loans); the
    /// services held, attendances, guests, conversions, walkouts, tithe.
    pub wr_report: u32,
    pub wr_flee: u32,
    pub wr_gossip: u32,
    pub wr_blackmail: u32,
    pub wr_learned: u32,
    pub bm_demands: u32,
    pub bm_paid: u32,
    pub bm_coins: i64,
    pub bm_silenced: u32,
    pub bm_reported: u32,
    pub hit_reposted: u32,
    pub hit_self: u32,
    pub hit_giveup: u32,
    pub snitch_grudges: u32,
    pub released_changed: u32,
    pub jail_absences: u32,
    pub jail_fired: u32,
    pub cell_fights: u32,
    pub cell_recruits: u32,
    pub cell_contacts: u32,
    pub cell_loans: u32,
    pub svc_held: u32,
    pub svc_attended: u32,
    pub svc_guests: u32,
    pub svc_converts: u32,
    pub svc_walkouts: u32,
    pub svc_tithe: i64,
    /// `Flow::Licence` and `Flow::Arms` coins.
    pub flow_licence: i64,
    pub flow_arms: i64,
    /// Phase 4, vice: standing StripClubs and Brothels; visits by kind and the companion visits at the Lounge; the venues' visit revenue and the tips (`Flow::Tip`).
    pub vice_venues: u32,
    pub strip_visits: u32,
    pub brothel_visits: u32,
    pub companion_visits: u32,
    pub vice_revenue: i64,
    pub tips: i64,
    /// Street pitches open, their visits and take; pimps assigned and their gangs' cut; the street's assaults.
    pub street_pitches: u32,
    pub street_visits: u32,
    pub street_take: i64,
    pub pimps: u32,
    pub pimp_cut: i64,
    pub street_assaults: u32,
    /// `Solicitation` reports filed at sweeps and arrests on them; Brothels closed by the law today.
    pub solicitation: u32,
    pub solicit_arrests: u32,
    pub brothel_closed: u32,
    /// Phase 4, fashion: standing Boutiques, their visits, clothing sold; clothing items alive and worn; clothing stolen.
    pub boutiques: u32,
    pub boutique_visits: u32,
    pub clothes_sold: u32,
    pub clothing: u32,
    pub clothes_worn: u32,
    pub clothes_stolen: u32,
    /// The `status` mean of living adults by class.
    pub status_corp: f32,
    pub status_street: f32,
    pub status_dreg: f32,
    /// Phase 5, the chip (§ 5.2): standing Dollhouses and their visits;
    /// contracts begun and ended (and ended early by an episode); contracts
    /// standing now; living adults with the `Chipped` trait; episodes of
    /// contracted workers; captives chipped at a gang's holding and the
    /// `Trafficking` reports filed.
    pub dollhouses: u32,
    pub dollhouse_visits: u32,
    pub chip_hired: u32,
    pub chip_ended: u32,
    pub chip_early: u32,
    pub chip_contracts: u32,
    pub chipped_living: u32,
    pub chip_episodes: u32,
    pub captives_chipped: u32,
    pub trafficking: u32,
    /// Phase 5, braindance (§ 5.3): standing BdStudios; units made, sold
    /// wholesale to Arcades, used at Arcade visits, sold to the World (and
    /// its coins); Recorders alive; recorded deeds; black units made and
    /// sold by dealers (and their coins).
    pub bd_studios: u32,
    pub bd_made: u32,
    pub bd_wholesale: u32,
    pub bd_viewed: u32,
    pub bd_exported: u32,
    pub bd_export_paid: i64,
    pub recorders: u32,
    pub recorded: u32,
    pub black_made: u32,
    pub black_sold: u32,
    pub black_revenue: i64,
    /// Phase 4's carry-over: street pitches swept outside a `Sweep` district.
    pub patrol_sweeps: u32,
    /// Phase 6, the gray market (§ 7): papers held (and the fake), sold by
    /// forgers, sold by a Clerk for a bribe, exposed at arrests; organs
    /// made, rotted, sold to Clinics, used at stabilisations, sold to the
    /// World (and its coins), seized at customs, in stock; typed gangs and
    /// their rips' kills; standing BlackClinics, `BlackMarket` reports,
    /// Ripperdocs struck off.
    pub papers_held: u32,
    pub fake_ids: u32,
    pub fake_sold: u32,
    pub papers_bribed: u32,
    pub exposures: u32,
    pub organs_made: u32,
    pub organs_rotted: u32,
    pub organs_to_clinics: u32,
    pub organs_used: u32,
    pub organs_exported: u32,
    pub organs_export_paid: i64,
    pub organs_seized: u32,
    pub organ_stock: u32,
    pub scav_gangs: u32,
    pub scav_kills: u32,
    pub black_clinics: u32,
    pub bm_reports: u32,
    pub struck_off: u32,
    /// Phase 6, faith (§ 8): followers (cult members included) and by
    /// creed row (the first four), cult members; tithes (`Flow::Tithe`),
    /// conversions, Preach attendances; apostates made and hunted now; the
    /// churches' purses.
    pub followers: u32,
    pub followers_c1: u32,
    pub followers_c2: u32,
    pub followers_c3: u32,
    pub followers_c4: u32,
    pub cult_members: u32,
    pub tithes: i64,
    pub conversions: u32,
    pub preach_attend: u32,
    pub apostates: u32,
    pub apostates_hunted: u32,
    pub church_purse: i64,
    /// Phase 7, life stages (§ 6): children by routine (Play, School,
    /// Chores, Street, Camp), child-days and school days, the fees, the
    /// street children taken to a camp, begged and stolen; Schools standing;
    /// elders (from `frail_from_years`), employed, taken in by kin today, on
    /// a Mission cot; the maimed living, maimed today, prosthetics fitted.
    pub ch_play: u32,
    pub ch_school: u32,
    pub ch_chores: u32,
    pub ch_street: u32,
    pub ch_camp: u32,
    pub child_days: u32,
    pub school_days: u32,
    pub school_fees: i64,
    pub street_takes: u32,
    pub child_begged: i64,
    pub child_thefts: u32,
    /// The pressure pass S6 (Dylan 2026-10-27): a death of a person under
    /// 18 refused at `World::kill_by` (every pick skips them first: a
    /// non-zero count is a pick site that missed, a broken-system signal).
    pub harm_blocked: u32,
    pub schools: u32,
    pub elders: u32,
    pub elders_employed: u32,
    pub elders_taken_in: u32,
    pub elders_on_cots: u32,
    pub maimed: u32,
    pub maimed_new: u32,
    pub prosthetics: u32,
    /// Lives at work phase 1 (docs/LIVES_AT_WORK.md § 1; deviation: the
    /// spec's `stats.current.jobs` home is this group): encounters served
    /// today by `ServeKind` (`ServeKind::ALL` order), the hour's unserved,
    /// staffed shifts of a serving row ended (the parity denominator), the
    /// cook's meals served and staff meals taken, regulars made, door
    /// refusals, ejections, and the madam's cut (`Flow::MadamCut`).
    /// (Left out of a save at zero, so an off save is byte-identical.)
    #[serde(skip_serializing_if = "served_is_zero")]
    pub served: [u32; 15],
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub served_waited: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub serve_shifts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub cooked: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub staff_meals: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub regulars_made: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub door_refusals: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub ejections: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub flow_madam_cut: i64,
    /// Lives at work phase 2 (docs/LIVES_AT_WORK.md § 3): HangOuts at one's
    /// own Home and at a friend's, those at home with the spouse as partner,
    /// spots left early for want of company, spouses reunited at midnight,
    /// visits made to hurt kin and to elder parents, newborns' carers named.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub home_hangouts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub friend_hangouts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub home_with_spouse: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub empty_leaves: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub home_empty_leaves: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub reunited: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub kin_visits: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub elder_visits: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub carers: u32,
    /// Lives at work phase 3 (docs/LIVES_AT_WORK.md § 2.2, § 3.5): the
    /// Camp's escapes, headcounts, the parents' visits, buy-outs
    /// and break-outs, the neglecting parents (a daily tally), the super's
    /// notices, door rent (coins), stops and tied move-ins, the buy-out
    /// coins (`Flow::Buyout`) and the School days taught (S6: the beating
    /// is gone, Dylan 2026-10-27).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub camp_escapes: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub camp_headcounts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub camp_visits: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub camp_buyouts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub camp_breakouts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub camp_neglect: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub notices: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub door_rent: i64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub super_stops: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub tied_housed: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub flow_buyout: i64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub taught_days: u32,
    /// Lives at work phase 4 (docs/LIVES_AT_WORK.md § 2.3): cab rides a
    /// driver's body drove and rides the Garage's off-screen cab took (no
    /// driver's body within `cab_wait_ticks`), fares dodged, parcels
    /// delivered and returned, the loaders' lifts, the owners' pilfer
    /// reports, clients found as bodies near the pitch, the pimps'
    /// collections and the coins passed up, corner deals, dealers' regulars
    /// made, Dealing witness rolls a lookout halved.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub cab_rides_driven: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub cab_rides_unmanned: u32,
    /// Round 3 S5 (OPEN-217): the cab drivers' cuts of the fares they drove
    /// (coins), clients a bouncer threw out for turning rough, refusals of
    /// the barred at a kept door, a harm contract heard of by its target
    /// (or his security).
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub cab_cuts: i64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub door_rough: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub door_barred: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub hit_heard: u32,
    /// Round 4 T4 (OPEN-217): a dancer's sets on the stage, a serving
    /// body's night told, the under-age turned away at a kept door, a
    /// body sent home from the shape-up, a Yard's foreman at the gate for it.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub sets_danced: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub serve_nights: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub door_under_age: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shape_up_home: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub foreman_gate: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fares_short: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub parcels_delivered: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub parcels_returned: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lifts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pilfer_reports: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub street_clients_near: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pimp_collections: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub pimp_pass_up: i64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub corner_deals: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub dealer_regulars: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lookout_halved: u32,
    // Lives at work phase 5 (§ 2.4, § 2.2's civic clerk): the tower's counters.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub meetings_held: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub meeting_attendees: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub decisions: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub beat_stops: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub interviews: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub bylines: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub byline_promotions: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub walk_ins_served: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub walk_ins_waited: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub walk_ins_dropped: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub contract_encounters: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fixer_collections: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub licences_named: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub bribes_named: u32,
    // Lives at work phase 6 (§ 4): careers and leaders. `lieutenants` is the
    // day's rank-1 headcount (its mean is the column's mean); `delegated_orders`
    // the leader-days delegated at midnight; the coins are the foremen's
    // premium paid and the payouts' cuts.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub band_refusals: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub foremen_premium: i64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub profession_rehires: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub foreman_hires: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub retirements: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub frail_exits: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub unretired: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lieutenants: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub delegated_orders: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lieutenant_raids: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub leader_cut_coins: i64,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub lieutenant_cut_coins: i64,
    // Lives at work phase 7 (§ 5.2, § 5.3): `stations` and `execs`,
    // `execs_spire` are the day's counts (Stations standing; corp execs, and
    // those housed at `exec_home_tier` or better); the rest are the day's
    // bookings at a Station (fines and nights served there among them), the
    // van's transfers, the arrests whose nearest Station was full (the
    // escort sent on) and the bookings with the Precinct full.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub stations: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub station_bookings: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub station_fines: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub station_nights: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub transfers: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub stations_full: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub precinct_full: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub execs: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub execs_spire: u32,
    // 8d (OPEN-129): the execs driven from the Spire to an Hq in another
    // district, and of those the rides in the corp's own vehicle (the rest
    // the Garage's off-screen cab, the fare paid by the corp).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub exec_rides: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub exec_rides_corp: u32,
    // The pre-view pass phase 2 (OPEN-137, `systems/shift.rs`): the stations
    // shifts closed today (Full and Coarse), their distinct spots worked
    // (summed), the breaks planned and the meal breaks of the shifts closed,
    // the day's longest block in minutes, and the shifts by their longest
    // block (<= 60, <= 90, <= 180, longer).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shift_stations: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shift_tiles: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shift_breaks: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shift_break_meals: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shift_block_max: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shift_long_60: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shift_long_90: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shift_long_180: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub shift_long_over: u32,
    // The pre-view pass phase 3 (docs/PREVIEW_PASS.md § 1): haul shifts
    // closed, trips unloaded, units carried (`[shift.haul] haul_load` a
    // trip), trips to another building than the post, the courier's
    // unloads with nobody present to sign (the parcel stays), counter
    // shifts closed, off-screen visits drawn into a counter body's shift,
    // and the Visit encounters served 06:00-18:00 and in the 21:00 hour.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub haul_shifts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub haul_trips: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub haul_units: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub haul_away: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub parcels_missed: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub counter_shifts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub counter_queued: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub served_day: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub served_21h: u32,
    // The pre-view pass phase 4 (P3, P8): encounters skipped for an absent
    // or sleeping counterpart (served or collected later, or off screen),
    // the sweeper's street stops and rounds walked, the shifts begun too far
    // from the Recycler to walk one (the old stand on the beat), the dealer's arms sales
    // made at a meeting with the off-screen buyer, those still booked at
    // midnight (both Statistical), meetings that lapsed, the street cuts
    // paid at the pimp's collection, Locate sightings not sold by a sleeper,
    // and the papers paid at the handover.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub presence_missed: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub sweep_stops: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub sweep_rounds: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub sweep_stands: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub arms_meet: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub arms_midnight: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub arms_missed: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub cut_at_meet: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub sight_asleep: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub papers_handover: u32,
    // The pre-view pass phase 5 (OPEN-139, OPEN-115, P12): segment ends
    // re-planned, escorts walked and driven by the van, the fastest escort
    // (tiles a minute, x 100), execs driven home, booked fares a driver's body carried to the door and paid
    // on arrival, fares the Garage's off-screen cab took, and why (no
    // free driver's body at the Garage, or none within `fare_wait_ticks`).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub leg_segments: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub escort_walks: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub escort_vans: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub escort_tpm_max: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub exec_rides_home: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fares_driven: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fares_unmanned: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fares_no_body: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fares_too_far: u32,
    // The pre-view pass phase 6 (P13-P15): a body leader's rounds begun,
    // their stops reached, the protection weeks collected there in person
    // and the stops with the owner out (the week stays owed), courts held,
    // takes handed to the leader at court, and orders given to a
    // lieutenant at court, at the lieutenant after the wait, and by word
    // past the cap (off screen).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lead_rounds: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lead_stops: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lead_collected: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub lead_owner_out: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub courts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub court_handovers: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub orders_court: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub orders_walked: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub orders_word: u32,
    // The island pass phase 3a (I6): the company beds standing and those
    // held at the midnight pass (gauges), the moves into a company bed and
    // the evictions from one after the grace (today).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub company_beds: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub company_housed: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub company_moved_in: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub company_evicted: u32,
    // The island pass phase 3b (I6): the company beds inside a workplace
    // (a gauge at the midnight pass), the blocks bought and the fallback
    // blocks built, the tenants given notice by a buyout and those evicted
    // at its end (today; `company_evicted` keeps the job lost).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub company_inside_beds: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub company_buyouts: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub company_built: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub company_notices: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub company_buyout_evicted: u32,
    // The island pass phase 4 (I2): the workers under commute pressure (a
    // gauge at the midnight pass), and today's households moved nearer work
    // by it (those into a company bed among them), the people they held,
    // and the workers who took a nearer post instead.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pressure_far: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pressure_moved: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pressure_company: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pressure_movers: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pressure_jobs: u32,
    // The island pass phase 8 (OPEN-168): a Builder's load set down at a far
    // site of his Yard (`[island_trades] site_day`; the rest of the shift
    // worked there).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub site_days: u32,
    // Island fix round F3 (OPEN-190): the stay-in HangOuts completed (the
    // own Home past the spot cap; `home_hangouts` counts them too).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub stay_in_hangouts: u32,
    // The pressure pass phase 3 (`systems::protection`): protections made on
    // held ground, leans refused, weeks missed; the punishments (a till
    // smashed or a stall kicked over, an owner beaten, a fire); the owner's
    // choice after (paid, reported, a rival's cover, sold or shut); members
    // bailed out and the bail paid; the gangs' treasuries at midnight
    // (median, max; snapshots).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_leaned: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_refused: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_missed: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_smash: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_beat: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_fire: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_pay: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_report: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_rival: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_sell: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub pr_bail: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub pr_bail_coins: i64,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub gang_treasury_med: i64,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub gang_treasury_max: i64,
    // OPEN-157: a contract's party arrived at the Fixer in person in its hours.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub broker_walks: u32,
    // The pressure pass phase 5b (`systems::crew`): raid results, duty, court, release, rest days.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_raid_results: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_raid_wounded: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_duty_posted: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_duty_kept: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_duty_missed: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_chose_gang: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_chose_job: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub crew_duty_pay: i64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_colours_lost: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_talked: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_talker_beaten: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_welcomes: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_promoted: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_court_quorum: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_court_failed: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_absorbed: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_folded: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_rest_nights: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_gang_loans: u32,
    // Pressure pass round 2, R5 (OPEN-214): a gang standing its paid duty
    // down (broke), a try-out turned away.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_stood_down: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub crew_tryout_refused: u32,
    // Pressure pass round 2, R1 (`systems::fares`): the fare dodger's ladder:
    // caught (both tiers), fined on the spot, beaten, banned, a ban broken,
    // caught off screen (of the caught), a banned rider turned off the platform.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fare_caught: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fare_fined: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fare_beaten: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fare_banned: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fare_ban_broken: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fare_stat: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fare_ban_walks: u32,
    /// Pressure pass round 3, S3: caught inside `beat_cooldown_days` of his
    /// last beating, turned off the platform instead.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub fare_turned_off: u32,
    /// Round 3 S3: arrests by the guard who saw the crime (his collar).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub law_collars: u32,
    // Pressure pass round 2, R3 (`systems::care`).
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub care_doc_cut: i64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_pawns: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_slates: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub care_slate_coins: i64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_asks_refused: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_asks_lent: u32,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub care_ask_coins: i64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_calls: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_strangers: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_beat: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_back_stabilised: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_woken: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_covered: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub care_uncovered: u32,
    // Pressure pass round 3, S1 (`[distance.choice]`, OPEN-221): the trips
    // not set out on past their reach (a muster sat out, a dealer off the
    // spot today, a kin visit too far, a hire not made).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub choice_muster_out: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub choice_deal_off: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub choice_visit_far: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub choice_hire_far: u32,
}

fn served_is_zero(v: &[u32; 15]) -> bool {
    v.iter().all(|&n| n == 0)
}

impl Jobs2Cols {
    /// Lives at work phase 1: every encounter served today.
    pub fn served_total(&self) -> u32 {
        self.served.iter().sum()
    }

    /// The header's names, comma-separated, in column order.
    pub fn header() -> String {
        [
            "flow_handling",
            "handling_leak",
            "flow_power",
            "power_fuel",
            "flow_build",
            "food_packed",
            "rations_made",
            "rations_sold",
            "rations_paid",
            "pilfered",
            "builds_started",
            "builds_done",
            "sites_open",
            "dark_districts",
            "outages",
            "jailers",
            "clerks",
            "depot_hands",
            "civic3_target",
            "office_workers",
            "hqs",
            "emp_produce",
            "emp_service",
            "emp_handling",
            "emp_contract",
            "emp_civic",
            "emp_overhead",
            "emp_resale",
            "emp_day",
            "emp_packing",
            "emp_freight",
            "emp_utility",
            "emp_construction",
            "emp_hq",
            "n_packing",
            "n_freight",
            "n_utility",
            "n_construction",
            "pitches",
            "pitches_vendor",
            "pitches_rickshaw",
            "pitches_tinker",
            "pitch_opened",
            "pitch_closed",
            "street_cut",
            "swept",
            "vendor_meals",
            "vendor_take",
            "day_posts_offered",
            "day_posts_filled",
            "day_posts_ended",
            "corner_queue",
            "day_labourers",
            "parcels",
            "parcel_fees",
            "cab_trips",
            "cab_fares",
            "rickshaw_trips",
            "rickshaw_fares",
            "tinker_repairs",
            "tinker_take",
            "tinker_botched",
            "plants_seized",
            "weapons",
            "carriers",
            "armoury",
            "precinct_arms",
            "gunsmith_stock",
            "licences",
            "weapon_wants",
            "weapons_made",
            "sold_legal",
            "sold_dealer",
            "issued",
            "seized",
            "crates_landed",
            "crates_seized",
            "fights",
            "fight_deaths",
            "armed_fights",
            "armed_deaths",
            "ua_reports",
            "licences_sold",
            "pickpockets",
            "stall_thefts",
            "home_thefts",
            "fenced",
            "misery_skips",
            "misery_fired",
            "misery_quits",
            "brawls",
            "misery_joins",
            "misery_converts",
            "uw_short",
            "uw_hold",
            "uw_hours",
            "uw_layoffs",
            "uw_loans",
            "uw_missed",
            "uw_beatings",
            "uw_seized",
            "uw_repaid",
            "uw_sold_corp",
            "uw_sold_gang",
            "uw_closed",
            "uw_reopened",
            "uw_afloat",
            "uw_walkouts",
            "uw_confronts",
            "uw_paid_up",
            "uw_confront_fights",
            "uw_reports",
            "uw_garnished",
            "uw_front_joins",
            "uw_front_hits",
            "uw_laundered",
            "door_scenes",
            "lonely_visits",
            "cab_hails",
            "fear_wants",
            "gang_arms_buys",
            "patched",
            "black_patients",
            "back_rooms",
            "vendor_bodies",
            "wr_report",
            "wr_flee",
            "wr_gossip",
            "wr_blackmail",
            "wr_learned",
            "bm_demands",
            "bm_paid",
            "bm_coins",
            "bm_silenced",
            "bm_reported",
            "hit_reposted",
            "hit_self",
            "hit_giveup",
            "snitch_grudges",
            "released_changed",
            "jail_absences",
            "jail_fired",
            "cell_fights",
            "cell_recruits",
            "cell_contacts",
            "cell_loans",
            "svc_held",
            "svc_attended",
            "svc_guests",
            "svc_converts",
            "svc_walkouts",
            "svc_tithe",
            "flow_licence",
            "flow_arms",
            "vice_venues",
            "strip_visits",
            "brothel_visits",
            "companion_visits",
            "vice_revenue",
            "tips",
            "street_pitches",
            "street_visits",
            "street_take",
            "pimps",
            "pimp_cut",
            "street_assaults",
            "solicitation",
            "solicit_arrests",
            "brothel_closed",
            "boutiques",
            "boutique_visits",
            "clothes_sold",
            "clothing",
            "clothes_worn",
            "clothes_stolen",
            "status_corp",
            "status_street",
            "status_dreg",
            "dollhouses",
            "dollhouse_visits",
            "chip_hired",
            "chip_ended",
            "chip_early",
            "chip_contracts",
            "chipped_living",
            "chip_episodes",
            "captives_chipped",
            "trafficking",
            "bd_studios",
            "bd_made",
            "bd_wholesale",
            "bd_viewed",
            "bd_exported",
            "bd_export_paid",
            "recorders",
            "recorded",
            "black_made",
            "black_sold",
            "black_revenue",
            "patrol_sweeps",
            "papers_held",
            "fake_ids",
            "fake_sold",
            "papers_bribed",
            "exposures",
            "organs_made",
            "organs_rotted",
            "organs_to_clinics",
            "organs_used",
            "organs_exported",
            "organs_export_paid",
            "organs_seized",
            "organ_stock",
            "scav_gangs",
            "scav_kills",
            "black_clinics",
            "bm_reports",
            "struck_off",
            "followers",
            "followers_c1",
            "followers_c2",
            "followers_c3",
            "followers_c4",
            "cult_members",
            "tithes",
            "conversions",
            "preach_attend",
            "apostates",
            "apostates_hunted",
            "church_purse",
            "ch_play",
            "ch_school",
            "ch_chores",
            "ch_street",
            "ch_camp",
            "child_days",
            "school_days",
            "school_fees",
            "street_takes",
            "child_begged",
            "child_thefts",
            "harm_blocked",
            "schools",
            "elders",
            "elders_employed",
            "elders_taken_in",
            "elders_on_cots",
            "maimed",
            "maimed_new",
            "prosthetics",
        ]
        .into_iter()
        .map(str::to_string)
        .chain(crate::components::ServeKind::ALL.iter().map(|k| format!("served_{}", k.label())))
        .chain(
            [
                "served_waited",
                "serve_shifts",
                "cooked",
                "staff_meals",
                "regulars_made",
                "door_refusals",
                "ejections",
                "flow_madam_cut",
                "home_hangouts",
                "friend_hangouts",
                "home_with_spouse",
                "empty_leaves",
                "home_empty_leaves",
                "reunited",
                "kin_visits",
                "elder_visits",
                "carers",
                "camp_escapes",
                "camp_headcounts",
                "camp_visits",
                "camp_buyouts",
                "camp_breakouts",
                "camp_neglect",
                "notices",
                "door_rent",
                "super_stops",
                "tied_housed",
                "flow_buyout",
                "taught_days",
                "cab_rides_driven",
                "cab_rides_unmanned",
                "cab_cuts",
                "door_rough",
                "door_barred",
                "hit_heard",
                "sets_danced",
                "serve_nights",
                "door_under_age",
                "shape_up_home",
                "foreman_gate",
                "fares_short",
                "parcels_delivered",
                "parcels_returned",
                "lifts",
                "pilfer_reports",
                "street_clients_near",
                "pimp_collections",
                "pimp_pass_up",
                "corner_deals",
                "dealer_regulars",
                "lookout_halved",
                "meetings_held",
                "meeting_attendees",
                "decisions",
                "beat_stops",
                "interviews",
                "bylines",
                "byline_promotions",
                "walk_ins_served",
                "walk_ins_waited",
                "walk_ins_dropped",
                "contract_encounters",
                "fixer_collections",
                "licences_named",
                "bribes_named",
                "band_refusals",
                "foremen_premium",
                "profession_rehires",
                "foreman_hires",
                "retirements",
                "frail_exits",
                "unretired",
                "lieutenants",
                "delegated_orders",
                "lieutenant_raids",
                "leader_cut_coins",
                "lieutenant_cut_coins",
                "stations",
                "station_bookings",
                "station_fines",
                "station_nights",
                "transfers",
                "stations_full",
                "precinct_full",
                "execs",
                "execs_spire",
                "exec_rides",
                "exec_rides_corp",
                "shift_stations",
                "shift_tiles",
                "shift_breaks",
                "shift_break_meals",
                "shift_block_max",
                "shift_long_60",
                "shift_long_90",
                "shift_long_180",
                "shift_long_over",
                "haul_shifts",
                "haul_trips",
                "haul_units",
                "haul_away",
                "parcels_missed",
                "counter_shifts",
                "counter_queued",
                "served_day",
                "served_21h",
                "presence_missed",
                "sweep_stops",
                "sweep_rounds",
                "sweep_stands",
                "arms_meet",
                "arms_midnight",
                "arms_missed",
                "cut_at_meet",
                "sight_asleep",
                "papers_handover",
                "leg_segments",
                "escort_walks",
                "escort_vans",
                "escort_tpm_max",
                "exec_rides_home",
                "fares_driven",
                "fares_unmanned",
                "fares_no_body",
                "fares_too_far",
                "lead_rounds",
                "lead_stops",
                "lead_collected",
                "lead_owner_out",
                "courts",
                "court_handovers",
                "orders_court",
                "orders_walked",
                "orders_word",
                "company_beds",
                "company_housed",
                "company_moved_in",
                "company_evicted",
                "company_inside_beds",
                "company_buyouts",
                "company_built",
                "company_notices",
                "company_buyout_evicted",
                "pressure_far",
                "pressure_moved",
                "pressure_company",
                "pressure_movers",
                "pressure_jobs",
                "site_days",
                "stay_in_hangouts",
                "pr_leaned",
                "pr_refused",
                "pr_missed",
                "pr_smash",
                "pr_beat",
                "pr_fire",
                "pr_pay",
                "pr_report",
                "pr_rival",
                "pr_sell",
                "pr_bail",
                "pr_bail_coins",
                "gang_treasury_med",
                "gang_treasury_max",
                "broker_walks",
                "crew_raid_results",
                "crew_raid_wounded",
                "crew_duty_posted",
                "crew_duty_kept",
                "crew_duty_missed",
                "crew_chose_gang",
                "crew_chose_job",
                "crew_duty_pay",
                "crew_colours_lost",
                "crew_talked",
                "crew_talker_beaten",
                "crew_welcomes",
                "crew_promoted",
                "crew_court_quorum",
                "crew_court_failed",
                "crew_absorbed",
                "crew_folded",
                "crew_rest_nights",
                "crew_gang_loans",
                "crew_stood_down",
                "crew_tryout_refused",
                "fare_caught",
                "fare_fined",
                "fare_beaten",
                "fare_banned",
                "fare_ban_broken",
                "fare_stat",
                "fare_ban_walks",
                "fare_turned_off",
                "law_collars",
                "care_doc_cut",
                "care_pawns",
                "care_slates",
                "care_slate_coins",
                "care_asks_refused",
                "care_asks_lent",
                "care_ask_coins",
                "care_calls",
                "care_strangers",
                "care_beat",
                "care_back_stabilised",
                "care_woken",
                "care_covered",
                "care_uncovered",
                "choice_muster_out",
                "choice_deal_off",
                "choice_visit_far",
                "choice_hire_far",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .collect::<Vec<String>>()
        .join(",")
    }

    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        [
            self.flow_handling.to_string(),
            self.handling_leak.to_string(),
            self.flow_power.to_string(),
            self.power_fuel.to_string(),
            self.flow_build.to_string(),
            self.food_packed.to_string(),
            self.rations_made.to_string(),
            self.rations_sold.to_string(),
            self.rations_paid.to_string(),
            self.pilfered.to_string(),
            self.builds_started.to_string(),
            self.builds_done.to_string(),
            self.sites_open.to_string(),
            self.dark_districts.to_string(),
            self.outages.to_string(),
            self.jailers.to_string(),
            self.clerks.to_string(),
            self.depot_hands.to_string(),
            self.civic3_target.to_string(),
            self.office_workers.to_string(),
            self.hqs.to_string(),
            self.emp_produce.to_string(),
            self.emp_service.to_string(),
            self.emp_handling.to_string(),
            self.emp_contract.to_string(),
            self.emp_civic.to_string(),
            self.emp_overhead.to_string(),
            self.emp_resale.to_string(),
            self.emp_day.to_string(),
            self.emp_packing.to_string(),
            self.emp_freight.to_string(),
            self.emp_utility.to_string(),
            self.emp_construction.to_string(),
            self.emp_hq.to_string(),
            self.n_packing.to_string(),
            self.n_freight.to_string(),
            self.n_utility.to_string(),
            self.n_construction.to_string(),
            self.pitches.to_string(),
            self.pitches_vendor.to_string(),
            self.pitches_rickshaw.to_string(),
            self.pitches_tinker.to_string(),
            self.pitch_opened.to_string(),
            self.pitch_closed.to_string(),
            self.street_cut.to_string(),
            self.swept.to_string(),
            self.vendor_meals.to_string(),
            self.vendor_take.to_string(),
            self.day_posts_offered.to_string(),
            self.day_posts_filled.to_string(),
            self.day_posts_ended.to_string(),
            self.corner_queue.to_string(),
            self.day_labourers.to_string(),
            self.parcels.to_string(),
            self.parcel_fees.to_string(),
            self.cab_trips.to_string(),
            self.cab_fares.to_string(),
            self.rickshaw_trips.to_string(),
            self.rickshaw_fares.to_string(),
            self.tinker_repairs.to_string(),
            self.tinker_take.to_string(),
            self.tinker_botched.to_string(),
            self.plants_seized.to_string(),
            self.weapons.to_string(),
            self.carriers.to_string(),
            self.armoury.to_string(),
            self.precinct_arms.to_string(),
            self.gunsmith_stock.to_string(),
            self.licences.to_string(),
            self.weapon_wants.to_string(),
            self.weapons_made.to_string(),
            self.sold_legal.to_string(),
            self.sold_dealer.to_string(),
            self.issued.to_string(),
            self.seized.to_string(),
            self.crates_landed.to_string(),
            self.crates_seized.to_string(),
            self.fights.to_string(),
            self.fight_deaths.to_string(),
            self.armed_fights.to_string(),
            self.armed_deaths.to_string(),
            self.ua_reports.to_string(),
            self.licences_sold.to_string(),
            self.pickpockets.to_string(),
            self.stall_thefts.to_string(),
            self.home_thefts.to_string(),
            self.fenced.to_string(),
            self.misery_skips.to_string(),
            self.misery_fired.to_string(),
            self.misery_quits.to_string(),
            self.brawls.to_string(),
            self.misery_joins.to_string(),
            self.misery_converts.to_string(),
            self.uw_short.to_string(),
            self.uw_hold.to_string(),
            self.uw_hours.to_string(),
            self.uw_layoffs.to_string(),
            self.uw_loans.to_string(),
            self.uw_missed.to_string(),
            self.uw_beatings.to_string(),
            self.uw_seized.to_string(),
            self.uw_repaid.to_string(),
            self.uw_sold_corp.to_string(),
            self.uw_sold_gang.to_string(),
            self.uw_closed.to_string(),
            self.uw_reopened.to_string(),
            self.uw_afloat.to_string(),
            self.uw_walkouts.to_string(),
            self.uw_confronts.to_string(),
            self.uw_paid_up.to_string(),
            self.uw_confront_fights.to_string(),
            self.uw_reports.to_string(),
            self.uw_garnished.to_string(),
            self.uw_front_joins.to_string(),
            self.uw_front_hits.to_string(),
            self.uw_laundered.to_string(),
            self.door_scenes.to_string(),
            self.lonely_visits.to_string(),
            self.cab_hails.to_string(),
            self.fear_wants.to_string(),
            self.gang_arms_buys.to_string(),
            self.patched.to_string(),
            self.black_patients.to_string(),
            self.back_rooms.to_string(),
            self.vendor_bodies.to_string(),
            self.wr_report.to_string(),
            self.wr_flee.to_string(),
            self.wr_gossip.to_string(),
            self.wr_blackmail.to_string(),
            self.wr_learned.to_string(),
            self.bm_demands.to_string(),
            self.bm_paid.to_string(),
            self.bm_coins.to_string(),
            self.bm_silenced.to_string(),
            self.bm_reported.to_string(),
            self.hit_reposted.to_string(),
            self.hit_self.to_string(),
            self.hit_giveup.to_string(),
            self.snitch_grudges.to_string(),
            self.released_changed.to_string(),
            self.jail_absences.to_string(),
            self.jail_fired.to_string(),
            self.cell_fights.to_string(),
            self.cell_recruits.to_string(),
            self.cell_contacts.to_string(),
            self.cell_loans.to_string(),
            self.svc_held.to_string(),
            self.svc_attended.to_string(),
            self.svc_guests.to_string(),
            self.svc_converts.to_string(),
            self.svc_walkouts.to_string(),
            self.svc_tithe.to_string(),
            self.flow_licence.to_string(),
            self.flow_arms.to_string(),
            self.vice_venues.to_string(),
            self.strip_visits.to_string(),
            self.brothel_visits.to_string(),
            self.companion_visits.to_string(),
            self.vice_revenue.to_string(),
            self.tips.to_string(),
            self.street_pitches.to_string(),
            self.street_visits.to_string(),
            self.street_take.to_string(),
            self.pimps.to_string(),
            self.pimp_cut.to_string(),
            self.street_assaults.to_string(),
            self.solicitation.to_string(),
            self.solicit_arrests.to_string(),
            self.brothel_closed.to_string(),
            self.boutiques.to_string(),
            self.boutique_visits.to_string(),
            self.clothes_sold.to_string(),
            self.clothing.to_string(),
            self.clothes_worn.to_string(),
            self.clothes_stolen.to_string(),
            format!("{:.3}", self.status_corp),
            format!("{:.3}", self.status_street),
            format!("{:.3}", self.status_dreg),
            self.dollhouses.to_string(),
            self.dollhouse_visits.to_string(),
            self.chip_hired.to_string(),
            self.chip_ended.to_string(),
            self.chip_early.to_string(),
            self.chip_contracts.to_string(),
            self.chipped_living.to_string(),
            self.chip_episodes.to_string(),
            self.captives_chipped.to_string(),
            self.trafficking.to_string(),
            self.bd_studios.to_string(),
            self.bd_made.to_string(),
            self.bd_wholesale.to_string(),
            self.bd_viewed.to_string(),
            self.bd_exported.to_string(),
            self.bd_export_paid.to_string(),
            self.recorders.to_string(),
            self.recorded.to_string(),
            self.black_made.to_string(),
            self.black_sold.to_string(),
            self.black_revenue.to_string(),
            self.patrol_sweeps.to_string(),
            self.papers_held.to_string(),
            self.fake_ids.to_string(),
            self.fake_sold.to_string(),
            self.papers_bribed.to_string(),
            self.exposures.to_string(),
            self.organs_made.to_string(),
            self.organs_rotted.to_string(),
            self.organs_to_clinics.to_string(),
            self.organs_used.to_string(),
            self.organs_exported.to_string(),
            self.organs_export_paid.to_string(),
            self.organs_seized.to_string(),
            self.organ_stock.to_string(),
            self.scav_gangs.to_string(),
            self.scav_kills.to_string(),
            self.black_clinics.to_string(),
            self.bm_reports.to_string(),
            self.struck_off.to_string(),
            self.followers.to_string(),
            self.followers_c1.to_string(),
            self.followers_c2.to_string(),
            self.followers_c3.to_string(),
            self.followers_c4.to_string(),
            self.cult_members.to_string(),
            self.tithes.to_string(),
            self.conversions.to_string(),
            self.preach_attend.to_string(),
            self.apostates.to_string(),
            self.apostates_hunted.to_string(),
            self.church_purse.to_string(),
            self.ch_play.to_string(),
            self.ch_school.to_string(),
            self.ch_chores.to_string(),
            self.ch_street.to_string(),
            self.ch_camp.to_string(),
            self.child_days.to_string(),
            self.school_days.to_string(),
            self.school_fees.to_string(),
            self.street_takes.to_string(),
            self.child_begged.to_string(),
            self.child_thefts.to_string(),
            self.harm_blocked.to_string(),
            self.schools.to_string(),
            self.elders.to_string(),
            self.elders_employed.to_string(),
            self.elders_taken_in.to_string(),
            self.elders_on_cots.to_string(),
            self.maimed.to_string(),
            self.maimed_new.to_string(),
            self.prosthetics.to_string(),
        ]
        .into_iter()
        .chain(self.served.iter().map(u32::to_string))
        .chain([
            self.served_waited.to_string(),
            self.serve_shifts.to_string(),
            self.cooked.to_string(),
            self.staff_meals.to_string(),
            self.regulars_made.to_string(),
            self.door_refusals.to_string(),
            self.ejections.to_string(),
            self.flow_madam_cut.to_string(),
            self.home_hangouts.to_string(),
            self.friend_hangouts.to_string(),
            self.home_with_spouse.to_string(),
            self.empty_leaves.to_string(),
            self.home_empty_leaves.to_string(),
            self.reunited.to_string(),
            self.kin_visits.to_string(),
            self.elder_visits.to_string(),
            self.carers.to_string(),
            self.camp_escapes.to_string(),
            self.camp_headcounts.to_string(),
            self.camp_visits.to_string(),
            self.camp_buyouts.to_string(),
            self.camp_breakouts.to_string(),
            self.camp_neglect.to_string(),
            self.notices.to_string(),
            self.door_rent.to_string(),
            self.super_stops.to_string(),
            self.tied_housed.to_string(),
            self.flow_buyout.to_string(),
            self.taught_days.to_string(),
            self.cab_rides_driven.to_string(),
            self.cab_rides_unmanned.to_string(),
            self.cab_cuts.to_string(),
            self.door_rough.to_string(),
            self.door_barred.to_string(),
            self.hit_heard.to_string(),
            self.sets_danced.to_string(),
            self.serve_nights.to_string(),
            self.door_under_age.to_string(),
            self.shape_up_home.to_string(),
            self.foreman_gate.to_string(),
            self.fares_short.to_string(),
            self.parcels_delivered.to_string(),
            self.parcels_returned.to_string(),
            self.lifts.to_string(),
            self.pilfer_reports.to_string(),
            self.street_clients_near.to_string(),
            self.pimp_collections.to_string(),
            self.pimp_pass_up.to_string(),
            self.corner_deals.to_string(),
            self.dealer_regulars.to_string(),
            self.lookout_halved.to_string(),
            self.meetings_held.to_string(),
            self.meeting_attendees.to_string(),
            self.decisions.to_string(),
            self.beat_stops.to_string(),
            self.interviews.to_string(),
            self.bylines.to_string(),
            self.byline_promotions.to_string(),
            self.walk_ins_served.to_string(),
            self.walk_ins_waited.to_string(),
            self.walk_ins_dropped.to_string(),
            self.contract_encounters.to_string(),
            self.fixer_collections.to_string(),
            self.licences_named.to_string(),
            self.bribes_named.to_string(),
            self.band_refusals.to_string(),
            self.foremen_premium.to_string(),
            self.profession_rehires.to_string(),
            self.foreman_hires.to_string(),
            self.retirements.to_string(),
            self.frail_exits.to_string(),
            self.unretired.to_string(),
            self.lieutenants.to_string(),
            self.delegated_orders.to_string(),
            self.lieutenant_raids.to_string(),
            self.leader_cut_coins.to_string(),
            self.lieutenant_cut_coins.to_string(),
            self.stations.to_string(),
            self.station_bookings.to_string(),
            self.station_fines.to_string(),
            self.station_nights.to_string(),
            self.transfers.to_string(),
            self.stations_full.to_string(),
            self.precinct_full.to_string(),
            self.execs.to_string(),
            self.execs_spire.to_string(),
            self.exec_rides.to_string(),
            self.exec_rides_corp.to_string(),
            self.shift_stations.to_string(),
            self.shift_tiles.to_string(),
            self.shift_breaks.to_string(),
            self.shift_break_meals.to_string(),
            self.shift_block_max.to_string(),
            self.shift_long_60.to_string(),
            self.shift_long_90.to_string(),
            self.shift_long_180.to_string(),
            self.shift_long_over.to_string(),
            self.haul_shifts.to_string(),
            self.haul_trips.to_string(),
            self.haul_units.to_string(),
            self.haul_away.to_string(),
            self.parcels_missed.to_string(),
            self.counter_shifts.to_string(),
            self.counter_queued.to_string(),
            self.served_day.to_string(),
            self.served_21h.to_string(),
            self.presence_missed.to_string(),
            self.sweep_stops.to_string(),
            self.sweep_rounds.to_string(),
            self.sweep_stands.to_string(),
            self.arms_meet.to_string(),
            self.arms_midnight.to_string(),
            self.arms_missed.to_string(),
            self.cut_at_meet.to_string(),
            self.sight_asleep.to_string(),
            self.papers_handover.to_string(),
            self.leg_segments.to_string(),
            self.escort_walks.to_string(),
            self.escort_vans.to_string(),
            self.escort_tpm_max.to_string(),
            self.exec_rides_home.to_string(),
            self.fares_driven.to_string(),
            self.fares_unmanned.to_string(),
            self.fares_no_body.to_string(),
            self.fares_too_far.to_string(),
            self.lead_rounds.to_string(),
            self.lead_stops.to_string(),
            self.lead_collected.to_string(),
            self.lead_owner_out.to_string(),
            self.courts.to_string(),
            self.court_handovers.to_string(),
            self.orders_court.to_string(),
            self.orders_walked.to_string(),
            self.orders_word.to_string(),
            self.company_beds.to_string(),
            self.company_housed.to_string(),
            self.company_moved_in.to_string(),
            self.company_evicted.to_string(),
            self.company_inside_beds.to_string(),
            self.company_buyouts.to_string(),
            self.company_built.to_string(),
            self.company_notices.to_string(),
            self.company_buyout_evicted.to_string(),
            self.pressure_far.to_string(),
            self.pressure_moved.to_string(),
            self.pressure_company.to_string(),
            self.pressure_movers.to_string(),
            self.pressure_jobs.to_string(),
            self.site_days.to_string(),
            self.stay_in_hangouts.to_string(),
            self.pr_leaned.to_string(),
            self.pr_refused.to_string(),
            self.pr_missed.to_string(),
            self.pr_smash.to_string(),
            self.pr_beat.to_string(),
            self.pr_fire.to_string(),
            self.pr_pay.to_string(),
            self.pr_report.to_string(),
            self.pr_rival.to_string(),
            self.pr_sell.to_string(),
            self.pr_bail.to_string(),
            self.pr_bail_coins.to_string(),
            self.gang_treasury_med.to_string(),
            self.gang_treasury_max.to_string(),
            self.broker_walks.to_string(),
            self.crew_raid_results.to_string(),
            self.crew_raid_wounded.to_string(),
            self.crew_duty_posted.to_string(),
            self.crew_duty_kept.to_string(),
            self.crew_duty_missed.to_string(),
            self.crew_chose_gang.to_string(),
            self.crew_chose_job.to_string(),
            self.crew_duty_pay.to_string(),
            self.crew_colours_lost.to_string(),
            self.crew_talked.to_string(),
            self.crew_talker_beaten.to_string(),
            self.crew_welcomes.to_string(),
            self.crew_promoted.to_string(),
            self.crew_court_quorum.to_string(),
            self.crew_court_failed.to_string(),
            self.crew_absorbed.to_string(),
            self.crew_folded.to_string(),
            self.crew_rest_nights.to_string(),
            self.crew_gang_loans.to_string(),
            self.crew_stood_down.to_string(),
            self.crew_tryout_refused.to_string(),
            self.fare_caught.to_string(),
            self.fare_fined.to_string(),
            self.fare_beaten.to_string(),
            self.fare_banned.to_string(),
            self.fare_ban_broken.to_string(),
            self.fare_stat.to_string(),
            self.fare_ban_walks.to_string(),
            self.fare_turned_off.to_string(),
            self.law_collars.to_string(),
            self.care_doc_cut.to_string(),
            self.care_pawns.to_string(),
            self.care_slates.to_string(),
            self.care_slate_coins.to_string(),
            self.care_asks_refused.to_string(),
            self.care_asks_lent.to_string(),
            self.care_ask_coins.to_string(),
            self.care_calls.to_string(),
            self.care_strangers.to_string(),
            self.care_beat.to_string(),
            self.care_back_stabilised.to_string(),
            self.care_woken.to_string(),
            self.care_covered.to_string(),
            self.care_uncovered.to_string(),
            self.choice_muster_out.to_string(),
            self.choice_deal_off.to_string(),
            self.choice_visit_far.to_string(),
            self.choice_hire_far.to_string(),
        ])
        .collect::<Vec<String>>()
        .join(",")
    }
}

/// Scale S5: the Ambient tier's day (`systems::ambient::close_day`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AmbientCols {
    /// Ambient flats (occupied or not, materialised or not).
    pub flats: u64,
    /// Occupied Ambient flats not yet materialised, and their adults.
    pub households: u64,
    pub adults: u64,
    /// Adults materialised today (every path), the labour pool's hires and
    /// the households materialised as arrivals.
    pub materialised: u32,
    pub hires: u32,
    pub arrivals: u32,
    /// Coins crossed in from the World with them today.
    pub coins_in: i64,
    /// The materialised adults alive at the day's close, and every adult
    /// materialised since day 0.
    pub living: u32,
    pub total: u64,
    /// The districts' labour pool and food demand estimates, summed.
    pub labour: f32,
    pub food: f32,
    /// M17 phase 1: the Ambient book's day (`AmbientDay`): food units wanted
    /// and bought, venue visits wanted and made, the coins it paid for each
    /// (`Flow::AmbientSale`), and its appetite.
    pub food_wanted: u32,
    pub food_bought: u32,
    pub visits_wanted: u32,
    pub visits_made: u32,
    pub paid_food: i64,
    pub paid_visits: i64,
    pub appetite: f32,
}

impl AmbientCols {
    /// The header's names, comma-separated, in column order.
    pub fn header() -> String {
        [
            "ambient_flats",
            "ambient_households",
            "ambient_adults",
            "ambient_materialised",
            "ambient_hires",
            "ambient_arrivals",
            "ambient_coins_in",
            "ambient_living",
            "ambient_total",
            "ambient_labour",
            "ambient_food",
            "ambient_food_wanted",
            "ambient_food_bought",
            "ambient_visits_wanted",
            "ambient_visits_made",
            "flow_ambient_food",
            "flow_ambient_visits",
            "ambient_appetite",
        ]
        .join(",")
    }

    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{:.1},{:.1},{},{},{},{},{},{},{:.3}",
            self.flats,
            self.households,
            self.adults,
            self.materialised,
            self.hires,
            self.arrivals,
            self.coins_in,
            self.living,
            self.total,
            self.labour,
            self.food,
            self.food_wanted,
            self.food_bought,
            self.visits_wanted,
            self.visits_made,
            self.paid_food,
            self.paid_visits,
            self.appetite
        )
    }
}

/// M17 phase 2: one parent's day (`systems::parents::close_day`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParentCols {
    pub treasury: i64,
    /// Income abroad at the last midnight.
    pub income: i64,
    pub health: f32,
    pub attitude: f32,
    pub rep: f32,
    pub branch_loss: f32,
    pub order: String,
    /// M17 phase 4.
    pub edge: f32,
}

/// M17 phase 2: the parents' group. The flows are the last midnight's
/// (the parents act at midnight); the crossings are the whole day's, every
/// account (the World's too).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutsideCols {
    /// Coins the branches sent home (net of the tariff), the tariff the
    /// Treasury kept, the subsidies paid and their count.
    pub remit: i64,
    pub tariff: i64,
    pub parent_fund: i64,
    pub subsidies: u32,
    /// Coins crossing into and out of the city today, every account.
    pub crossed_in: i64,
    pub crossed_out: i64,
    /// Parents failing (income under upkeep for a week).
    pub failing: u32,
    /// M17 phase 3: bodies (the day's `Outside.today`; Outsiders living at
    /// the close and the State's guards).
    pub outsiders_living: u32,
    pub outsiders_arrived: u32,
    pub outsiders_left: u32,
    pub settled: u32,
    pub reinforcements: u32,
    pub buybacks: u32,
    pub retakes_posted: u32,
    pub retakes_won: u32,
    pub branches_founded: u32,
    pub parents_founded: u32,
    pub abandoned: u32,
    pub brothers_arrived: u32,
    pub gangs_refounded: u32,
    pub state_guards: u32,
    pub state_grants: u32,
    pub mandates: u32,
    pub withdrawals: u32,
    /// M17 phase 4: the weapons and scrip (the day's; `sanctions_active`
    /// and `scrip_out` at the close).
    pub uplink_runs: i64,
    pub uplink_breached: i64,
    pub data_shipped: i64,
    pub sanctions_new: i64,
    pub sanctions_active: i64,
    pub campaigns: i64,
    pub parents_dead: i64,
    pub scrip_out: i64,
    pub scrip_paid: i64,
    pub scrip_spent: i64,
    pub scrip_converted: i64,
    pub scrip_wiped: i64,
    /// Coins in from, and out to, the parents today (every flow: the
    /// balance of trade's parents term).
    pub parents_in: i64,
    pub parents_out: i64,
    /// M17 phase 4b: the succession (the day's; `wars_active` at the close).
    pub successions: u32,
    pub civil_wars: u32,
    pub wars_active: u32,
    pub war_loser_dead: u32,
    pub war_loser_fled: u32,
    pub war_loser_splinter: u32,
    pub war_loser_jailed: u32,
    /// M17 phase 5: missions abroad (the day's; `away` at the close), the
    /// cast promoted and arrived, Blackwall breaches.
    pub abroad_posted: u32,
    pub abroad_done: u32,
    pub abroad_won: u32,
    pub abroad_dead: u32,
    pub away: u32,
    pub promoted: u32,
    pub cast_arrived: u32,
    pub breaches: u32,
    pub accounts: Vec<ParentCols>,
}

impl OutsideCols {
    /// The header's names for `n` parents, comma-separated.
    pub fn header(n: usize) -> String {
        let mut v: Vec<String> = [
            "outside_remit",
            "outside_tariff",
            "outside_parent_fund",
            "outside_subsidies",
            "outside_in",
            "outside_out",
            "outside_net",
            "parents_failing",
            "outsiders_living",
            "outsiders_arrived",
            "outsiders_left",
            "settled",
            "reinforcements",
            "buybacks",
            "retakes_posted",
            "retakes_won",
            "branches_founded",
            "parents_founded",
            "abandoned",
            "brothers_arrived",
            "gangs_refounded",
            "state_guards",
            "state_grants",
            "mandates",
            "withdrawals",
            "uplink_runs",
            "uplink_breached",
            "data_shipped",
            "sanctions_new",
            "sanctions_active",
            "campaigns",
            "parents_dead",
            "scrip_out",
            "scrip_paid",
            "scrip_spent",
            "scrip_converted",
            "scrip_wiped",
            "parents_in",
            "parents_out",
            "successions",
            "civil_wars",
            "wars_active",
            "war_loser_dead",
            "war_loser_fled",
            "war_loser_splinter",
            "war_loser_jailed",
            "abroad_posted",
            "abroad_done",
            "abroad_won",
            "abroad_dead",
            "away",
            "promoted_cast",
            "cast_arrived",
            "blackwall_breaches",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for i in 1..=n {
            for c in ["treasury", "income", "health", "attitude", "rep", "loss", "order", "edge"] {
                v.push(format!("o{i}_{c}"));
            }
        }
        v.join(",")
    }

    pub fn csv(&self) -> String {
        let mut v = vec![
            self.remit.to_string(),
            self.tariff.to_string(),
            self.parent_fund.to_string(),
            self.subsidies.to_string(),
            self.crossed_in.to_string(),
            self.crossed_out.to_string(),
            (self.crossed_in - self.crossed_out).to_string(),
            self.failing.to_string(),
            self.outsiders_living.to_string(),
            self.outsiders_arrived.to_string(),
            self.outsiders_left.to_string(),
            self.settled.to_string(),
            self.reinforcements.to_string(),
            self.buybacks.to_string(),
            self.retakes_posted.to_string(),
            self.retakes_won.to_string(),
            self.branches_founded.to_string(),
            self.parents_founded.to_string(),
            self.abandoned.to_string(),
            self.brothers_arrived.to_string(),
            self.gangs_refounded.to_string(),
            self.state_guards.to_string(),
            self.state_grants.to_string(),
            self.mandates.to_string(),
            self.withdrawals.to_string(),
            self.uplink_runs.to_string(),
            self.uplink_breached.to_string(),
            self.data_shipped.to_string(),
            self.sanctions_new.to_string(),
            self.sanctions_active.to_string(),
            self.campaigns.to_string(),
            self.parents_dead.to_string(),
            self.scrip_out.to_string(),
            self.scrip_paid.to_string(),
            self.scrip_spent.to_string(),
            self.scrip_converted.to_string(),
            self.scrip_wiped.to_string(),
            self.parents_in.to_string(),
            self.parents_out.to_string(),
            self.successions.to_string(),
            self.civil_wars.to_string(),
            self.wars_active.to_string(),
            self.war_loser_dead.to_string(),
            self.war_loser_fled.to_string(),
            self.war_loser_splinter.to_string(),
            self.war_loser_jailed.to_string(),
            self.abroad_posted.to_string(),
            self.abroad_done.to_string(),
            self.abroad_won.to_string(),
            self.abroad_dead.to_string(),
            self.away.to_string(),
            self.promoted.to_string(),
            self.cast_arrived.to_string(),
            self.breaches.to_string(),
        ];
        for a in &self.accounts {
            v.push(a.treasury.to_string());
            v.push(a.income.to_string());
            v.push(format!("{:.3}", a.health));
            v.push(format!("{:.3}", a.attitude));
            v.push(format!("{:.3}", a.rep));
            v.push(format!("{:.3}", a.branch_loss));
            v.push(a.order.clone());
            v.push(format!("{:.3}", a.edge));
        }
        v.join(",")
    }
}

/// L2 (plan L34): the six leisure kinds' CSV names (`BuildingKind::LEISURE` order).
pub const LEISURE_SLOTS: [&str; 6] = ["club", "arcade", "noodle_bar", "fight_pit", "den", "lounge"];

/// L2 (plan L34): the living city's CSV columns, in header order: spec
/// §§ 1-3 and the plan's additions. Phase 1 fills the § 1 columns; the
/// rest stay zero until their phase.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LivingCols {
    pub flow_leisure: i64,
    /// Net to houses: losses minus wins.
    pub flow_gamble: i64,
    pub flow_gamble_win: i64,
    pub flow_tribute: i64,
    pub flow_export: i64,
    /// The day's wage bill of the public-works hires.
    pub flow_public_works: i64,
    /// Σ `flow_wages` ÷ max(Σ `flow_dole`, 1), the day's.
    pub wage_dole_ratio: f32,
    /// Employed adults ÷ adults.
    pub employed_share: f32,
    /// Standing venues per leisure kind (`LEISURE_SLOTS`).
    pub venues: [u32; 6],
    /// Visits today per leisure kind.
    pub visits: [u32; 6],
    pub fab_parts: u32,
    pub scrap_parts: u32,
    /// Σ the asset sellers' `Flow::Import` coins today.
    pub parts_imported: i64,
    pub works_jobs: u32,
    pub upkeep_mult: f32,
    pub outside_inbound: i64,
    pub outside_minted: i64,
    // § 2 (phase 2).
    pub fun_mean: f32,
    pub fun_satisfied_share: f32,
    pub hangouts: u32,
    pub hangout_contacts_mean: f32,
    pub fronts: u32,
    pub collected: u32,
    pub preached: u32,
    /// Per district slot (`DISTRICT_SLOTS`).
    pub street_density: Vec<f32>,
    // § 3 (phase 4).
    pub fv_killed: u32,
    pub fv_assaulted: u32,
    pub fv_robbed: u32,
    pub fv_abducted: u32,
    pub fv_bound: u32,
    pub fv_unknown: u32,
    pub fv_capped: u32,
    pub fv_bound_wrong: u32,
    pub kill_rate_body: f32,
    pub kill_rate_stat: f32,
    pub kill_rate_body_civ: f32,
    pub kill_rate_stat_civ: f32,
    // Phase 5 gate probes (not CSV columns; the L2 gate reads them).
    /// Coins the Statistical leisure pass (`leisure::stat_daily`) paid today.
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub stat_spend: i64,
    /// The island pass phase 10 (not a CSV column): the midnight pass's
    /// hits not made because their source had no free hand or budget left
    /// (`fviolence::daily`; 0 with `[fviolence.hands]` absent).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fv_hands_capped: u32,
    /// Parts `assets::parts_market` moved today from a Fab, from the Recycler.
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub parts_sold_fab: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub parts_sold_recycler: u32,
    /// Review fix: street dice's peer coins (`Flow::StreetDice`), a CSV
    /// column after the budget columns (appended; not in `csv()`).
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub flow_street_dice: i64,
    /// L2 shadow fixes (not CSV columns): shifts paid pro rata after a cut (item 2), no-show
    /// dismissals (item 11), commitments that held a plan against a new
    /// winner (items 1-4, 15), tier dwells that kept a body (item 7).
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fix_pro_rata: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fix_noshow: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fix_held: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub fix_dwell: u32,
}

fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

impl LivingCols {
    /// The header's names, comma-separated, in column order.
    pub fn header() -> String {
        let mut h: Vec<String> = [
            "flow_leisure",
            "flow_gamble",
            "flow_gamble_win",
            "flow_tribute",
            "flow_export",
            "flow_public_works",
            "wage_dole_ratio",
            "employed_share",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        h.extend(LEISURE_SLOTS.iter().map(|k| format!("venues_{k}")));
        h.extend(LEISURE_SLOTS.iter().map(|k| format!("visits_{k}")));
        h.extend(
            [
                "fab_parts",
                "scrap_parts",
                "parts_imported",
                "works_jobs",
                "upkeep_mult",
                "outside_inbound",
                "outside_minted",
                "fun_mean",
                "fun_satisfied_share",
                "hangouts",
                "hangout_contacts_mean",
                "fronts",
                "collected",
                "preached",
            ]
            .iter()
            .map(|s| s.to_string()),
        );
        h.extend((1..=DISTRICT_SLOTS).map(|i| format!("d{i}_street_density")));
        h.extend(
            [
                "fv_killed",
                "fv_assaulted",
                "fv_robbed",
                "fv_abducted",
                "fv_bound",
                "fv_unknown",
                "fv_capped",
                "fv_bound_wrong",
                "kill_rate_body",
                "kill_rate_stat",
                "kill_rate_body_civ",
                "kill_rate_stat_civ",
            ]
            .iter()
            .map(|s| s.to_string()),
        );
        h.join(",")
    }

    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        let mut v: Vec<String> = vec![
            self.flow_leisure.to_string(),
            self.flow_gamble.to_string(),
            self.flow_gamble_win.to_string(),
            self.flow_tribute.to_string(),
            self.flow_export.to_string(),
            self.flow_public_works.to_string(),
            format!("{:.3}", self.wage_dole_ratio),
            format!("{:.3}", self.employed_share),
        ];
        v.extend(self.venues.iter().map(|x| x.to_string()));
        v.extend(self.visits.iter().map(|x| x.to_string()));
        v.extend([
            self.fab_parts.to_string(),
            self.scrap_parts.to_string(),
            self.parts_imported.to_string(),
            self.works_jobs.to_string(),
            format!("{:.2}", self.upkeep_mult),
            self.outside_inbound.to_string(),
            self.outside_minted.to_string(),
            format!("{:.3}", self.fun_mean),
            format!("{:.3}", self.fun_satisfied_share),
            self.hangouts.to_string(),
            format!("{:.3}", self.hangout_contacts_mean),
            self.fronts.to_string(),
            self.collected.to_string(),
            self.preached.to_string(),
        ]);
        v.extend(
            (0..self.street_density.len().max(DISTRICT_SLOTS))
                .map(|i| format!("{:.3}", self.street_density.get(i).copied().unwrap_or(0.0))),
        );
        v.extend([
            self.fv_killed.to_string(),
            self.fv_assaulted.to_string(),
            self.fv_robbed.to_string(),
            self.fv_abducted.to_string(),
            self.fv_bound.to_string(),
            self.fv_unknown.to_string(),
            self.fv_capped.to_string(),
            self.fv_bound_wrong.to_string(),
            format!("{:.3}", self.kill_rate_body),
            format!("{:.3}", self.kill_rate_stat),
            format!("{:.3}", self.kill_rate_body_civ),
            format!("{:.3}", self.kill_rate_stat_civ),
        ]);
        v.join(",")
    }
}

/// M15 W43: the word's CSV columns, in header order: the spec's § 10 list,
/// the plan's additions, per gang slot dread and heat, per corp slot
/// honour, standing and competence, the Law's competence, the two ledger
/// flows. Phase 1 fills `rumours_heard`, `second_hand_share`,
/// `known_by_killers_median`, `distorted`, `contradicted`,
/// `rumour_hops_max`, `pool_reach`, the gang axes and the corp honour and
/// standing; the rest are zero until their phase.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WordCols {
    pub rumours_heard: u32,
    pub second_hand_share: f32,
    pub known_by_killers_median: f32,
    pub distorted: u32,
    pub grudges: u32,
    pub grudges_inherited: u32,
    pub hunts: u32,
    pub hunts_active: u32,
    pub avenged: u32,
    pub revenge_kills: u32,
    pub chain_max: u32,
    pub vendettas_open: u32,
    pub stories: u32,
    pub planted: u32,
    pub buried: u32,
    pub poached: u32,
    pub talent_lost: u32,
    pub skill_rare_share: f32,
    pub extort_success: u32,
    pub rep_flips: u32,
    pub rumour_hops_max: u32,
    pub pool_reach: f32,
    pub contradicted: u32,
    pub silenced: u32,
    pub hunts_failed: u32,
    pub hunts_abandoned: u32,
    pub guard_body: u32,
    pub expelled: u32,
    pub contracts_lost_honour: u32,
    pub extort_tries: u32,
    /// Per gang slot (`GANG_SLOTS`): dread, heat.
    pub gangs: Vec<[f32; 2]>,
    /// Per corp slot (`CORP_SLOTS`): honour, standing, competence.
    pub corps: Vec<[f32; 3]>,
    pub law_competence: f32,
    pub flow_ads: i64,
    pub flow_plant: i64,
}

impl WordCols {
    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        let gangs: Vec<String> = (0..GANG_SLOTS)
            .map(|i| {
                let [d, h] = self.gangs.get(i).copied().unwrap_or_default();
                format!("{d:.3},{h:.3}")
            })
            .collect();
        let corps: Vec<String> = (0..self.corps.len().max(CORP_SLOTS))
            .map(|i| {
                let [h, s, c] = self.corps.get(i).copied().unwrap_or_default();
                format!("{h:.3},{s:.3},{c:.3}")
            })
            .collect();
        format!(
            "{},{:.3},{:.1},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{:.3},{},{}",
            self.rumours_heard,
            self.second_hand_share,
            self.known_by_killers_median,
            self.distorted,
            self.grudges,
            self.grudges_inherited,
            self.hunts,
            self.hunts_active,
            self.avenged,
            self.revenge_kills,
            self.chain_max,
            self.vendettas_open,
            self.stories,
            self.planted,
            self.buried,
            self.poached,
            self.talent_lost,
            self.skill_rare_share,
            self.extort_success,
            self.rep_flips,
            self.rumour_hops_max,
            self.pool_reach,
            self.contradicted,
            self.silenced,
            self.hunts_failed,
            self.hunts_abandoned,
            self.guard_body,
            self.expelled,
            self.contracts_lost_honour,
            self.extort_tries,
            gangs.join(","),
            corps.join(","),
            self.law_competence,
            self.flow_ads,
            self.flow_plant,
        )
    }
}

/// L2 phase 3 (L34, spec § 4): the LOD budget's and the churn's columns.
/// `tier_held` is every sentenced agent and `tier_held_body` those of them
/// with a body; with `[lod] budget` on, `tier_full`, `tier_coarse` and
/// `tier_stat` exclude the sentenced (off: as M15). `gang_bodies` and
/// `gang_stat` are free members by tier at the day's end; `stat_*` the
/// Statistical GangWork days that acted; `aborts` the plan failures
/// (`step_agent`'s `Failed`, `plan_for`'s unplannable), by the failing
/// step: Scavenge, Sleep, CheckIn, a full non-bed door (`aborts_seat`);
/// `rough_sleeps` the failed bed steps turned into a street Sleep (L30);
/// `scavenge_dry` the dry Scavenge hours made `Done` (L29).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BudgetCols {
    pub tier_held: u32,
    pub tier_held_body: u32,
    pub held_fed: u32,
    pub gang_bodies: u32,
    pub gang_stat: u32,
    pub stat_extorts: u32,
    pub stat_claims: u32,
    pub stat_deals: u32,
    pub aborts: u32,
    pub aborts_scavenge: u32,
    pub aborts_sleep: u32,
    pub aborts_checkin: u32,
    pub aborts_seat: u32,
    pub rough_sleeps: u32,
    pub scavenge_dry: u32,
}

impl BudgetCols {
    pub fn is_zero(&self) -> bool {
        *self == BudgetCols::default()
    }

    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.tier_held,
            self.tier_held_body,
            self.held_fed,
            self.gang_bodies,
            self.gang_stat,
            self.stat_extorts,
            self.stat_claims,
            self.stat_deals,
            self.aborts,
            self.aborts_scavenge,
            self.aborts_sleep,
            self.aborts_checkin,
            self.aborts_seat,
            self.rough_sleeps,
            self.scavenge_dry,
        )
    }
}

/// M16a (plan C38): the contract records' CSV columns, in header order
/// (counters of a game abstraction: records posted, taken, settled).
/// `contracts_open`, `regulars`, `live_parties`, `live_queued`,
/// `guards_on_take`, `escrow_held`, `escrow_leak`, `escrow_stuck` and the per-Fixer slots
/// are day-end snapshots; the rest are daily counters (zero until their
/// phase: `hits_squad`, `strikes_declined_pol`, `sold_out` phase 2;
/// `contract_cleared`, `accessory*`, `interrogations*`, `guards_on_take`,
/// `fixer_runs`, `f*_heat` phase 3; `live_queued` phase 4).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContractCols {
    pub contracts_open: u32,
    pub contracts_posted: u32,
    pub contracts_fulfilled: u32,
    pub contracts_failed: u32,
    pub contracts_expired: u32,
    pub contracts_cancelled: u32,
    pub reneged: u32,
    /// Posted per kind (`ContractKind::ALL` order).
    pub k_posted: [u32; 4],
    /// Fulfilled per kind.
    pub k_done: [u32; 4],
    pub hits_done: u32,
    pub hits_squad: u32,
    /// Fulfilled-or-taken solo Hits whose solo estimate was below `squad_below`.
    pub hits_solo_weak: u32,
    pub strikes_declined_pol: u32,
    pub sold_out: u32,
    /// Hits fulfilled (live and ledger).
    pub contract_murders: u32,
    pub contract_cleared: u32,
    pub contract_holes: u32,
    pub contract_hole_wrong: u32,
    pub accessory: u32,
    pub interrogations: u32,
    pub interrogations_won: u32,
    pub bounties_paid: u32,
    pub guards_on_take: u32,
    pub fixer_runs: u32,
    pub regulars: u32,
    pub live_parties: u32,
    pub live_queued: u32,
    pub escrow_held: i64,
    pub escrow_leak: i64,
    /// Review fix (probe): escrow held by closed records (must be 0).
    pub escrow_stuck: i64,
    /// Per Fixer slot (by building id).
    pub f_heat: [f32; 4],
    pub f_income: [i64; 4],
    pub flow_escrow: i64,
    pub flow_payout: i64,
    pub flow_fixer_cut: i64,
    /// Review fix (phase 3): brain postings refused at `max_open` (a full
    /// board drops them silently otherwise).
    pub contracts_refused_full: u32,
}

impl ContractCols {
    pub fn is_zero(&self) -> bool {
        *self == ContractCols::default()
    }

    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        let mut out = format!(
            "{},{},{},{},{},{},{}",
            self.contracts_open,
            self.contracts_posted,
            self.contracts_fulfilled,
            self.contracts_failed,
            self.contracts_expired,
            self.contracts_cancelled,
            self.reneged,
        );
        for k in 0..4 {
            out.push_str(&format!(",{},{}", self.k_posted[k], self.k_done[k]));
        }
        out.push_str(&format!(
            ",{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.hits_done,
            self.hits_squad,
            self.hits_solo_weak,
            self.strikes_declined_pol,
            self.sold_out,
            self.contract_murders,
            self.contract_cleared,
            self.contract_holes,
            self.contract_hole_wrong,
            self.accessory,
            self.interrogations,
            self.interrogations_won,
            self.bounties_paid,
            self.guards_on_take,
            self.fixer_runs,
            self.regulars,
            self.live_parties,
            self.live_queued,
            self.escrow_held,
            self.escrow_leak,
            self.escrow_stuck,
            self.f_heat.iter().map(|h| format!("{h:.3}")).collect::<Vec<_>>().join(","),
        ));
        for v in self.f_income {
            out.push_str(&format!(",{v}"));
        }
        out.push_str(&format!(
            ",{},{},{},{}",
            self.flow_escrow, self.flow_payout, self.flow_fixer_cut, self.contracts_refused_full
        ));
        out
    }
}

/// M14 V43: the plane's CSV columns, in header order. `nodes`, `labs`,
/// `decks`, `cameras`, `ice_mean_corp`, `data_held` and the per-corp tiers
/// and Data are day-end snapshots; the rest are daily counters (zero until
/// their phase).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VirtCols {
    pub nodes: u32,
    pub labs: u32,
    pub decks: u32,
    pub runs: u32,
    pub runs_ok: u32,
    pub data_made: u32,
    pub data_stolen: u32,
    pub data_wiped: u32,
    pub data_sold: u32,
    pub ledger_hacks: u32,
    pub doors_hacked: u32,
    pub traced: u32,
    pub fried: u32,
    pub flatlined: u32,
    pub hack_arrests: u32,
    pub ice_mean_corp: f32,
    pub ice_spend: i64,
    pub runs_bounced: u32,
    pub runs_captured: u32,
    pub runs_dumped: u32,
    pub hack_arrests_chair: u32,
    pub robots_turned: u32,
    pub blinded: u32,
    pub cameras: u32,
    pub sightings: u32,
    pub ice_raised: u32,
    pub ice_lowered: u32,
    pub tech_gained: u32,
    pub tech_lost: u32,
    pub research_spent: u32,
    pub data_held: u32,
    /// Per corp slot (`CORP_SLOTS`): Chrome, Deck, Industry tiers and Data held.
    pub corps: Vec<[u32; 4]>,
    pub flow_data: i64,
    pub flow_hack: i64,
    pub flow_ice_upkeep: i64,
    pub flow_research: i64,
    pub flow_terminal: i64,
}

impl VirtCols {
    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        let corps: Vec<String> = (0..self.corps.len().max(CORP_SLOTS))
            .map(|i| {
                let [c, d, n, data] = self.corps.get(i).copied().unwrap_or_default();
                format!("{c},{d},{n},{data}")
            })
            .collect();
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.nodes,
            self.labs,
            self.decks,
            self.runs,
            self.runs_ok,
            self.data_made,
            self.data_stolen,
            self.data_wiped,
            self.data_sold,
            self.ledger_hacks,
            self.doors_hacked,
            self.traced,
            self.fried,
            self.flatlined,
            self.hack_arrests,
            self.ice_mean_corp,
            self.ice_spend,
            self.runs_bounced,
            self.runs_captured,
            self.runs_dumped,
            self.hack_arrests_chair,
            self.robots_turned,
            self.blinded,
            self.cameras,
            self.sightings,
            self.ice_raised,
            self.ice_lowered,
            self.tech_gained,
            self.tech_lost,
            self.research_spent,
            self.data_held,
            corps.join(","),
            self.flow_data,
            self.flow_hack,
            self.flow_ice_upkeep,
            self.flow_research,
            self.flow_terminal,
        )
    }
}

impl DayRow {
    /// Scale S2: the row's district slots, [`DISTRICT_SLOTS`] or the run's
    /// districts when more (`districts` holds one entry per district).
    pub fn district_slots(&self) -> usize {
        self.districts.len().clamp(DISTRICT_SLOTS, MAX_DISTRICTS)
    }

    pub fn new(day: u64) -> Self {
        DayRow {
            day,
            season: Season::of_day(day),
            population: 0,
            employed: 0,
            homeless: 0,
            jailed: 0,
            gang_members: 0,
            food_market: 0,
            food_warehouse: 0,
            food_pantry: 0,
            price: 0,
            treasury: 0,
            mean_hunger: 0.0,
            mean_mood: 0.0,
            thefts: 0,
            arrests: 0,
            deaths_starvation: 0,
            deaths_old_age: 0,
            deaths_violence: 0,
            births: 0,
            immigrants: 0,
            emigrants: 0,
            burials: 0,
            goal_changes: 0,
            goal_changes_per_agent: 0.0,
            holes_opened: 0,
            holes_bound: 0,
            holes_unknown: 0,
            deaths_violence_offscreen: 0,
            holes_open: 0,
            tier_full: 0,
            tier_coarse: 0,
            tier_stat: 0,
            evictions: 0,
            rent_paid: 0,
            rent_short: 0,
            housed: 0,
            flow_food: 0,
            flow_drink: 0,
            flow_wages: 0,
            flow_rent: 0,
            flow_upkeep: 0,
            flow_wholesale: 0,
            flow_overflow: 0,
            flow_restock: 0,
            flow_contract: 0,
            flow_tax: 0,
            flow_dole: 0,
            flow_other: 0,
            wallets: 0,
            wallet_gini: 0.0,
            wallet_top10: 0.0,
            corps: Vec::new(),
            acquisitions: 0,
            bankruptcies: 0,
            monopolies: 0,
            foundings: 0,
            incorporations: 0,
            strikes: 0,
            unrest_corp: 0.0,
            unrest_street: 0.0,
            unrest_dreg: 0.0,
            class_corp: 0,
            class_street: 0,
            class_dreg: 0,
            happiness_street: 0.0,
            districts: Vec::new(),
            hotel_nights: 0,
            squatters: 0,
            derelicts: 0,
            vagrancy: 0,
            riots: 0,
            crossfire: 0,
            gangs: 0,
            raids_into_cover: 0,
            vehicles_moto: 0,
            vehicles_car: 0,
            vehicles_truck: 0,
            vehicles_flyer: 0,
            truck_hauls: 0,
            walk_hauls: 0,
            commute_tpt_walk: 0.0,
            commute_tpt_drive: 0.0,
            chrome_installs: 0,
            chrome_agents: 0,
            mean_sanity: 0.0,
            episodes: 0,
            hooked: 0,
            stims_dealt: 0,
            stims_legal: 0,
            dealing_reports: 0,
            repos: 0,
            impounds: 0,
            crashes: 0,
            crash_deaths: 0,
            vehicle_thefts: 0,
            chops: 0,
            abductions: 0,
            stripped: 0,
            robots: 0,
            flow_asset: 0,
            flow_asset_upkeep: 0,
            flow_finance: 0,
            flow_import: 0,
            flow_stims: 0,
            flow_parts: 0,
            flow_treatment: 0,
            overdoses: 0,
            harvests: 0,
            stripped_window: 0,
            gang_income: 0,
            gang_income_dealing: 0,
            episodes_by_law: 0,
            treatments: 0,
            detoxes: 0,
            virt: VirtCols::default(),
            word: WordCols::default(),
            living: LivingCols::default(),
            budget: BudgetCols::default(),
            contract: ContractCols::default(),
            econ: EconCols::default(),
            jobs: JobsCols::default(),
            health: HealthCols::default(),
            hit: HitCols::default(),
            leverage: LeverageCols::default(),
            captive: CaptiveCols::default(),
            wounded: WoundedCols::default(),
            gov: GovCols::default(),
            transit: TransitCols::default(),
            ambient: None,
            outside: None,
            jobs2: None,
            ticks_per_sec: 0.0,
        }
    }

    /// One CSV line matching [`CSV_HEADER`], without a trailing newline.
    pub fn csv_row(&self) -> String {
        // Hunger as sickness: the health group rides with the jobs group.
        let jobs_health = format!("{},{}", self.jobs.csv(), self.health.csv());
        // M16b: the leverage group rides with the hit group.
        let hit_lev = format!(
            "{},{},{},{},{}",
            self.hit.csv(),
            self.leverage.csv(),
            self.captive.csv(),
            self.wounded.csv(),
            self.gov.csv()
        );
        // Scale S5: the Ambient group rides with the transit group (only on a
        // world with an Ambient tier).
        let transit_ambient = match &self.ambient {
            Some(a) => format!("{},{}", self.transit.csv(), a.csv()),
            None => self.transit.csv(),
        };
        // M17 phase 2: the parents' group after it (only with the parents on).
        let transit_ambient = match &self.outside {
            Some(o) => format!("{transit_ambient},{}", o.csv()),
            None => transit_ambient,
        };
        // Jobs & lives: its group after that (only with `[jobs2]` on).
        let transit_ambient = match &self.jobs2 {
            Some(j) => format!("{transit_ambient},{}", j.csv()),
            None => transit_ambient,
        };
        let corps: Vec<String> = (0..self.corps.len().max(CORP_SLOTS))
            .map(|i| match self.corps.get(i).copied().flatten() {
                Some((t, o)) => format!("{t},{o}"),
                None => "0,-".to_string(),
            })
            .collect();
        let districts: Vec<String> = (0..self.district_slots())
            .map(|i| {
                let (cov, ctrl, litter, unrest, crime, guards) = self.districts.get(i).copied().unwrap_or_default();
                format!("{cov:.3},{ctrl},{litter:.3},{unrest:.3},{crime:.3},{guards}")
            })
            .collect();
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.0}",
            self.day,
            self.season,
            self.population,
            self.employed,
            self.homeless,
            self.jailed,
            self.gang_members,
            self.food_market,
            self.food_warehouse,
            self.food_pantry,
            self.price,
            self.treasury,
            self.thefts,
            self.arrests,
            self.deaths_starvation,
            self.deaths_old_age,
            self.deaths_violence,
            self.births,
            self.immigrants,
            self.emigrants,
            self.burials,
            self.mean_hunger,
            self.mean_mood,
            self.goal_changes_per_agent,
            self.holes_opened,
            self.holes_open,
            self.holes_bound,
            self.holes_unknown,
            self.deaths_violence_offscreen,
            self.tier_full,
            self.tier_coarse,
            self.tier_stat,
            self.evictions,
            self.rent_paid,
            self.rent_short,
            self.housed,
            self.flow_food,
            self.flow_drink,
            self.flow_wages,
            self.flow_rent,
            self.flow_upkeep,
            self.flow_wholesale,
            self.flow_overflow,
            self.flow_restock,
            self.flow_contract,
            self.flow_tax,
            self.flow_dole,
            self.flow_other,
            self.wallets,
            self.wallet_gini,
            self.wallet_top10,
            corps.join(","),
            self.acquisitions,
            self.bankruptcies,
            self.monopolies,
            self.foundings,
            self.incorporations,
            self.strikes,
            self.unrest_corp,
            self.unrest_street,
            self.unrest_dreg,
            self.class_corp,
            self.class_street,
            self.class_dreg,
            self.happiness_street,
            districts.join(","),
            self.hotel_nights,
            self.squatters,
            self.derelicts,
            self.vagrancy,
            self.riots,
            self.crossfire,
            self.gangs,
            self.vehicles_moto,
            self.vehicles_car,
            self.vehicles_truck,
            self.vehicles_flyer,
            self.truck_hauls,
            self.walk_hauls,
            self.commute_tpt_walk,
            self.commute_tpt_drive,
            self.chrome_installs,
            self.chrome_agents,
            self.mean_sanity,
            self.episodes,
            self.hooked,
            self.stims_dealt,
            self.stims_legal,
            self.dealing_reports,
            self.repos,
            self.impounds,
            self.crashes,
            self.crash_deaths,
            self.vehicle_thefts,
            self.chops,
            self.abductions,
            self.stripped,
            self.robots,
            self.flow_asset,
            self.flow_asset_upkeep,
            self.flow_finance,
            self.flow_import,
            self.flow_stims,
            self.flow_parts,
            self.flow_treatment,
            self.overdoses,
            self.harvests,
            self.stripped_window,
            self.gang_income,
            self.gang_income_dealing,
            self.episodes_by_law,
            self.treatments,
            self.detoxes,
            self.virt.csv(),
            self.word.csv(),
            self.living.csv(),
            self.budget.csv(),
            self.living.flow_street_dice,
            self.contract.csv(),
            self.econ.csv_slots(self.district_slots()),
            jobs_health,
            hit_lev,
            transit_ambient,
            self.ticks_per_sec,
        )
    }
}

/// The Real economy (docs/ECONOMY_V2.md § 7, plan E45): `EconCols`, after
/// M16a's `ContractCols` and before `ticks_per_sec`. Phase 1 fills the
/// market's columns (`flow_import_out`, `flow_customs`, `flow_migrant_*`,
/// `flow_fence`, `trade_balance(_30)`, `appetite_*`, `bid_food`, `ask_food`,
/// `food_imported`, `food_exported`, `parts_exported`, `data_exported`,
/// `export_paid`, `outside_outbound`, `world_treasury`, `coin_identity`,
/// `city_owned_buildings`, `d*_starving`, `d*_dregs`); the rest stay zero
/// until their phase (2: inputs, property, wages, hiring, inflows,
/// `grow_world`; 3a: `tax_rate`, `recycler_till`; 3b: Missions, `d*_meals`;
/// 3c: the camps). Snapshot columns read at the day's end; flows are the
/// day's sums.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EconCols {
    pub flow_import_out: i64,
    pub flow_customs: i64,
    pub flow_inputs: i64,
    pub flow_property: i64,
    pub flow_donate: i64,
    /// `export_paid + flow_migrant_in + flow_fence + flow_ambient_* − flow_import_out − flow_inputs − flow_migrant_out`
    /// (M17 phase 1 added the Ambient book's purchases: 0 without Ambient flats; phase 2 the
    /// parents' subsidies in less the remittances out).
    pub trade_balance: i64,
    pub trade_balance_30: i64,
    pub appetite_food: f32,
    pub appetite_parts: f32,
    pub appetite_data: f32,
    /// Today's first-unit Food bid.
    pub bid_food: f32,
    pub ask_food: i64,
    pub food_imported: u32,
    pub food_exported: u32,
    pub wage_mult_mean: f32,
    pub wage_gross_mean: f32,
    pub vacancies_open: u32,
    pub laid_off: u32,
    pub pop_inflow_wages: i64,
    pub pop_inflow_other: i64,
    pub tax_rate: f32,
    pub mission_meals: u32,
    pub mission_cots: u32,
    pub mission_purse: i64,
    pub flow_migrant_in: i64,
    pub flow_migrant_out: i64,
    pub flow_fence: i64,
    pub flow_alms: i64,
    pub flow_camp_food: i64,
    pub outside_outbound: i64,
    pub world_treasury: i64,
    /// `total_coins + Σ outside treasuries − minted`: constant to the coin with the market on.
    pub coin_identity: i64,
    pub parts_exported: u32,
    pub data_exported: u32,
    pub export_paid: i64,
    pub grow_world: u32,
    /// Standing niche buildings on the city's deed (foreclosed and seeded).
    pub city_owned_buildings: u32,
    pub recycler_till: i64,
    pub children_taken: u32,
    pub camp_children: u32,
    pub camp_output: u32,
    pub camp_unfed: u32,
    pub camp_released: u32,
    pub camp_scandals: u32,
    /// Per district (`DistrictId::index`; Scale S2: every district, 8 before;
    /// the CSV prints the run's district slots): Mission meals to its
    /// residents, `Starving` events by the agent's district, Dreg-class
    /// adults by Home district.
    #[serde(deserialize_with = "crate::components::district_slots_any_len")]
    pub d_meals: [u32; MAX_DISTRICTS],
    #[serde(deserialize_with = "crate::components::district_slots_any_len")]
    pub d_starving: [u32; MAX_DISTRICTS],
    #[serde(deserialize_with = "crate::components::district_slots_any_len")]
    pub d_dregs: [u32; MAX_DISTRICTS],
}

impl EconCols {
    pub fn is_zero(&self) -> bool {
        *self == EconCols::default()
    }

    /// The header's names, comma-separated, in column order.
    pub fn header() -> String {
        let mut h: Vec<String> = [
            "flow_import_out",
            "flow_customs",
            "flow_inputs",
            "flow_property",
            "flow_donate",
            "trade_balance",
            "trade_balance_30",
            "appetite_food",
            "appetite_parts",
            "appetite_data",
            "bid_food",
            "ask_food",
            "food_imported",
            "food_exported",
            "wage_mult_mean",
            "wage_gross_mean",
            "vacancies_open",
            "laid_off",
            "pop_inflow_wages",
            "pop_inflow_other",
            "tax_rate",
            "mission_meals",
            "mission_cots",
            "mission_purse",
            "flow_migrant_in",
            "flow_migrant_out",
            "flow_fence",
            "flow_alms",
            "flow_camp_food",
            "outside_outbound",
            "world_treasury",
            "coin_identity",
            "parts_exported",
            "data_exported",
            "export_paid",
            "grow_world",
            "city_owned_buildings",
            "recycler_till",
            "children_taken",
            "camp_children",
            "camp_output",
            "camp_unfed",
            "camp_released",
            "camp_scandals",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for i in 1..=DISTRICT_SLOTS {
            h.push(format!("d{i}_meals"));
            h.push(format!("d{i}_starving"));
            h.push(format!("d{i}_dregs"));
        }
        h.join(",")
    }

    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        self.csv_slots(DISTRICT_SLOTS)
    }

    /// [`EconCols::csv`] with `districts` district slots (at most `MAX_DISTRICTS`).
    pub fn csv_slots(&self, districts: usize) -> String {
        let mut v: Vec<String> = vec![
            self.flow_import_out.to_string(),
            self.flow_customs.to_string(),
            self.flow_inputs.to_string(),
            self.flow_property.to_string(),
            self.flow_donate.to_string(),
            self.trade_balance.to_string(),
            self.trade_balance_30.to_string(),
            format!("{:.3}", self.appetite_food),
            format!("{:.3}", self.appetite_parts),
            format!("{:.3}", self.appetite_data),
            format!("{:.3}", self.bid_food),
            self.ask_food.to_string(),
            self.food_imported.to_string(),
            self.food_exported.to_string(),
            format!("{:.3}", self.wage_mult_mean),
            format!("{:.3}", self.wage_gross_mean),
            self.vacancies_open.to_string(),
            self.laid_off.to_string(),
            self.pop_inflow_wages.to_string(),
            self.pop_inflow_other.to_string(),
            format!("{:.3}", self.tax_rate),
            self.mission_meals.to_string(),
            self.mission_cots.to_string(),
            self.mission_purse.to_string(),
            self.flow_migrant_in.to_string(),
            self.flow_migrant_out.to_string(),
            self.flow_fence.to_string(),
            self.flow_alms.to_string(),
            self.flow_camp_food.to_string(),
            self.outside_outbound.to_string(),
            self.world_treasury.to_string(),
            self.coin_identity.to_string(),
            self.parts_exported.to_string(),
            self.data_exported.to_string(),
            self.export_paid.to_string(),
            self.grow_world.to_string(),
            self.city_owned_buildings.to_string(),
            self.recycler_till.to_string(),
            self.children_taken.to_string(),
            self.camp_children.to_string(),
            self.camp_output.to_string(),
            self.camp_unfed.to_string(),
            self.camp_released.to_string(),
            self.camp_scandals.to_string(),
        ];
        for i in 0..districts.min(MAX_DISTRICTS) {
            v.push(self.d_meals[i].to_string());
            v.push(self.d_starving[i].to_string());
            v.push(self.d_dregs[i].to_string());
        }
        v.join(",")
    }
}

/// Jobs v2 (plan J34): the jobs pass's CSV group, after `EconCols` and
/// before `ticks_per_sec`. P5a: `civic_payroll` (the day's wages paid by the
/// Treasury: police, Sanitation and every other city role) and `flow_fines`
/// (the day's Vagrancy fines, also in `flow_other`), and the day-end
/// `levers.{guard_count, sanitation_count}` (the civic budget's headcounts,
/// J16; not in J34's list, added to read the budget). Flows are the day's sums.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobsCols {
    pub civic_payroll: i64,
    pub flow_fines: i64,
    pub guard_count: u16,
    pub sanitation_count: u16,
    /// Jobs v2 J19 (P6): the day's arrivals drawn by the Harris-Todaro pull
    /// (a spouse counted with its partner).
    pub migrants_pull: u32,
    /// J20 (P6): the day's arrivals holding an offer (a vacancy open `offer_days`).
    pub migrants_offer: u32,
    /// P3 (J34): Job holders in a configured trade at the day's end.
    pub trades_employed: u32,
    /// P3 (J14): Super rounds walked today (`SuperRound`).
    pub super_rounds: u32,
    /// The surplus credit (`[treasury] credit_max`): coins the Treasury paid
    /// today as a credit on wages at a negative tax rate (`flow_tax` stays
    /// the gross tax, the civic budget's receipts).
    pub tax_credit: i64,
}

impl JobsCols {
    pub fn is_zero(&self) -> bool {
        *self == JobsCols::default()
    }

    /// The header's names, comma-separated, in column order.
    pub fn header() -> String {
        [
            "civic_payroll",
            "flow_fines",
            "guard_count",
            "sanitation_count",
            "migrants_pull",
            "migrants_offer",
            "trades_employed",
            "super_rounds",
            "tax_credit",
        ]
        .join(",")
    }

    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        [
            self.civic_payroll.to_string(),
            self.flow_fines.to_string(),
            self.guard_count.to_string(),
            self.sanitation_count.to_string(),
            self.migrants_pull.to_string(),
            self.migrants_offer.to_string(),
            self.trades_employed.to_string(),
            self.super_rounds.to_string(),
            self.tax_credit.to_string(),
        ]
        .join(",")
    }
}

/// Hunger as sickness (docs/HUNGER_SICKNESS.md § 9): the health group,
/// after `JobsCols` and before `ticks_per_sec`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HealthCols {
    pub meals_real: u32,
    pub meals_synth: u32,
    pub meals_dive: u32,
    pub meals_alms: u32,
    /// Citizens at or over `mal_onset` / adults at or over `tox_onset` at midnight.
    pub malnourished: u32,
    pub toxic: u32,
    pub mean_malnutrition: f32,
    pub mean_toxicity: f32,
    /// Adults off sick today (rolled at midnight).
    pub sick_days: u32,
    pub cancer_new: u32,
    pub cancer_diagnosed: u32,
    pub cancer_cured: u32,
    pub deaths_disease: u32,
    /// Scrap meals into the bins today.
    pub bin_meals_in: u32,
    pub bin_contests: u32,
    pub bin_shakedowns: u32,
}

impl HealthCols {
    pub fn is_zero(&self) -> bool {
        *self == HealthCols::default()
    }

    pub fn header() -> String {
        "meals_real,meals_synth,meals_dive,meals_alms,malnourished,toxic,mean_malnutrition,mean_toxicity,sick_days,cancer_new,cancer_diagnosed,cancer_cured,deaths_disease,bin_meals_in,bin_contests,bin_shakedowns".to_string()
    }

    pub fn csv(&self) -> String {
        [
            self.meals_real.to_string(),
            self.meals_synth.to_string(),
            self.meals_dive.to_string(),
            self.meals_alms.to_string(),
            self.malnourished.to_string(),
            self.toxic.to_string(),
            format!("{:.4}", self.mean_malnutrition),
            format!("{:.4}", self.mean_toxicity),
            self.sick_days.to_string(),
            self.cancer_new.to_string(),
            self.cancer_diagnosed.to_string(),
            self.cancer_cured.to_string(),
            self.deaths_disease.to_string(),
            self.bin_meals_in.to_string(),
            self.bin_contests.to_string(),
            self.bin_shakedowns.to_string(),
        ]
        .join(",")
    }
}

/// The Hit market (docs/HIT_MARKET.md § 7): organic records (motive not
/// `None`). Counts are the day's; `hm_price_sum` and `hm_deposit_sum` sum
/// the day's organic postings (means = sum ÷ posted); `hm_wants` counts
/// wants written for the walk to a Fixer (H9) and `hm_commissions` those
/// posted there; `hm_unaffordable` counts tries short of the deposit;
/// `hm_refused` relatives who would not chip in; `hm_pooled` the coins they
/// did put in; `hm_recanted` beaten witnesses who took a report back (H15);
/// `hm_cold` reports gone cold on a dead witness (H14).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HitCols {
    pub hm_posted_revenge: u32,
    pub hm_posted_rival: u32,
    pub hm_posted_culprit: u32,
    pub hm_posted_silence: u32,
    pub hm_posted_debt: u32,
    pub hm_posted_protect: u32,
    pub hm_wants: u32,
    pub hm_unaffordable: u32,
    pub hm_refused: u32,
    pub hm_pooled: i64,
    pub hm_price_sum: i64,
    pub hm_deposit_sum: i64,
    pub hm_hits_done: u32,
    pub hm_beats_done: u32,
    pub hm_stiffed: u32,
    pub hm_recanted: u32,
    pub hm_cold: u32,
    pub hm_commissions: u32,
}

impl HitCols {
    pub fn is_zero(&self) -> bool {
        *self == HitCols::default()
    }

    /// One organic posting by motive.
    pub fn note_posted(&mut self, m: crate::contract::Motive) {
        use crate::contract::Motive;
        match m {
            Motive::None => {}
            Motive::Revenge => self.hm_posted_revenge += 1,
            Motive::Rival => self.hm_posted_rival += 1,
            Motive::Culprit => self.hm_posted_culprit += 1,
            Motive::Silence => self.hm_posted_silence += 1,
            Motive::Debt => self.hm_posted_debt += 1,
            Motive::Protect => self.hm_posted_protect += 1,
        }
    }

    pub fn header() -> String {
        [
            "hm_posted_revenge",
            "hm_posted_rival",
            "hm_posted_culprit",
            "hm_posted_silence",
            "hm_posted_debt",
            "hm_posted_protect",
            "hm_wants",
            "hm_unaffordable",
            "hm_refused",
            "hm_pooled",
            "hm_price_sum",
            "hm_deposit_sum",
            "hm_hits_done",
            "hm_beats_done",
            "hm_stiffed",
            "hm_recanted",
            "hm_cold",
            "hm_commissions",
        ]
        .join(",")
    }

    pub fn csv(&self) -> String {
        [
            self.hm_posted_revenge.to_string(),
            self.hm_posted_rival.to_string(),
            self.hm_posted_culprit.to_string(),
            self.hm_posted_silence.to_string(),
            self.hm_posted_debt.to_string(),
            self.hm_posted_protect.to_string(),
            self.hm_wants.to_string(),
            self.hm_unaffordable.to_string(),
            self.hm_refused.to_string(),
            self.hm_pooled.to_string(),
            self.hm_price_sum.to_string(),
            self.hm_deposit_sum.to_string(),
            self.hm_hits_done.to_string(),
            self.hm_beats_done.to_string(),
            self.hm_stiffed.to_string(),
            self.hm_recanted.to_string(),
            self.hm_cold.to_string(),
            self.hm_commissions.to_string(),
        ]
        .join(",")
    }
}

/// M16b § 4 (phase 1): leverage's CSV columns, after the Hit market's and
/// before transit's. `coercions_active`, `protection_active`,
/// `blackmail_active` and `tribute_active` are day-end snapshots; the rest
/// are daily counters. `lev_*_tries`/`_ok` count the moves by leverage
/// kind (the gang shakedown, M15's `extort_tries`, apart); `silenced_threat`
/// reports withdrawn under a threat; `k_persuade_*` the `Persuade` kind's
/// records (M16a's `k_*` slots are `ContractCols`').
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LeverageCols {
    pub coercions_active: u32,
    pub lev_threat_tries: u32,
    pub lev_threat_ok: u32,
    pub lev_secret_tries: u32,
    pub lev_secret_ok: u32,
    pub lev_debt_tries: u32,
    pub lev_debt_ok: u32,
    pub coercions_broken: u32,
    pub protection_made: u32,
    pub protection_paid: u32,
    pub protection_coins: i64,
    pub protection_active: u32,
    pub blackmail_made: u32,
    pub blackmail_active: u32,
    pub secrets_lapsed: u32,
    pub silenced_threat: u32,
    pub coerced_jobs: u32,
    pub coerced_done: u32,
    pub poach_threats: u32,
    pub parleys: u32,
    pub parley_fails: u32,
    pub tribute_active: u32,
    pub fake_jobs: u32,
    pub frauds_filed: u32,
    pub k_persuade_posted: u32,
    pub k_persuade_done: u32,
    pub flow_coerced: i64,
}

impl LeverageCols {
    pub fn is_zero(&self) -> bool {
        *self == LeverageCols::default()
    }

    /// One move by leverage kind.
    pub fn note_try(&mut self, k: crate::contract::LeverageKind, ok: bool) {
        use crate::contract::LeverageKind;
        let (t, o) = match k {
            LeverageKind::Threat => (&mut self.lev_threat_tries, &mut self.lev_threat_ok),
            LeverageKind::Secret => (&mut self.lev_secret_tries, &mut self.lev_secret_ok),
            LeverageKind::Debt => (&mut self.lev_debt_tries, &mut self.lev_debt_ok),
            // M16b phase 2: counted in `CaptiveCols`.
            LeverageKind::Hostage => return,
        };
        *t += 1;
        if ok {
            *o += 1;
        }
    }

    pub fn header() -> String {
        [
            "coercions_active",
            "lev_threat_tries",
            "lev_threat_ok",
            "lev_secret_tries",
            "lev_secret_ok",
            "lev_debt_tries",
            "lev_debt_ok",
            "coercions_broken",
            "protection_made",
            "protection_paid",
            "protection_coins",
            "protection_active",
            "blackmail_made",
            "blackmail_active",
            "secrets_lapsed",
            "silenced_threat",
            "coerced_jobs",
            "coerced_done",
            "poach_threats",
            "parleys",
            "parley_fails",
            "tribute_active",
            "fake_jobs",
            "frauds_filed",
            "k_persuade_posted",
            "k_persuade_done",
            "flow_coerced",
        ]
        .join(",")
    }

    pub fn csv(&self) -> String {
        [
            self.coercions_active.to_string(),
            self.lev_threat_tries.to_string(),
            self.lev_threat_ok.to_string(),
            self.lev_secret_tries.to_string(),
            self.lev_secret_ok.to_string(),
            self.lev_debt_tries.to_string(),
            self.lev_debt_ok.to_string(),
            self.coercions_broken.to_string(),
            self.protection_made.to_string(),
            self.protection_paid.to_string(),
            self.protection_coins.to_string(),
            self.protection_active.to_string(),
            self.blackmail_made.to_string(),
            self.blackmail_active.to_string(),
            self.secrets_lapsed.to_string(),
            self.silenced_threat.to_string(),
            self.coerced_jobs.to_string(),
            self.coerced_done.to_string(),
            self.poach_threats.to_string(),
            self.parleys.to_string(),
            self.parley_fails.to_string(),
            self.tribute_active.to_string(),
            self.fake_jobs.to_string(),
            self.frauds_filed.to_string(),
            self.k_persuade_posted.to_string(),
            self.k_persuade_done.to_string(),
            self.flow_coerced.to_string(),
        ]
        .join(",")
    }
}

/// M16b phase 2: the captives' and the new kinds' CSV columns, after
/// leverage's. `captives_held`, `prison_pop` (captives held at a Private
/// Prison) and `prisons` are day-end snapshots; the rest daily counters.
/// `hostages_freed` counts every release (`ransomed`: a demand met,
/// `rescued_captives`: a Rescue's win, `escaped`: the hold gone, the rest
/// at expiry); `flow_ransom` the coins paid for a hostage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptiveCols {
    pub captives_held: u32,
    pub prison_pop: u32,
    pub prisons: u32,
    pub hostages_taken: u32,
    pub hostages_freed: u32,
    pub ransomed: u32,
    pub rescued_captives: u32,
    pub escaped: u32,
    pub hostages_killed: u32,
    pub harvest_handovers: u32,
    pub captives_unfed: u32,
    pub lev_hostage_tries: u32,
    pub lev_hostage_ok: u32,
    /// Posted per M16b phase-2 kind (`ContractKind::PHASE2` order).
    pub k_posted: [u32; 6],
    /// Fulfilled per phase-2 kind.
    pub k_done: [u32; 6],
    pub fabricated: u32,
    pub exposed: u32,
    pub steal_runs: u32,
    pub flow_ransom: i64,
}

impl CaptiveCols {
    pub fn is_zero(&self) -> bool {
        *self == CaptiveCols::default()
    }

    /// The slot of a phase-2 kind (`None` for the others).
    pub fn slot(k: crate::contract::ContractKind) -> Option<usize> {
        crate::contract::ContractKind::PHASE2.iter().position(|&x| x == k)
    }

    pub fn header() -> String {
        [
            "captives_held",
            "prison_pop",
            "prisons",
            "hostages_taken",
            "hostages_freed",
            "ransomed",
            "rescued_captives",
            "escaped",
            "hostages_killed",
            "harvest_handovers",
            "captives_unfed",
            "lev_hostage_tries",
            "lev_hostage_ok",
            "k_abduct_posted",
            "k_abduct_done",
            "k_rescue_posted",
            "k_rescue_done",
            "k_escort_posted",
            "k_escort_done",
            "k_steal_posted",
            "k_steal_done",
            "k_deliver_posted",
            "k_deliver_done",
            "k_spin_posted",
            "k_spin_done",
            "fabricated",
            "exposed",
            "steal_runs",
            "flow_ransom",
        ]
        .join(",")
    }

    pub fn csv(&self) -> String {
        [
            self.captives_held.to_string(),
            self.prison_pop.to_string(),
            self.prisons.to_string(),
            self.hostages_taken.to_string(),
            self.hostages_freed.to_string(),
            self.ransomed.to_string(),
            self.rescued_captives.to_string(),
            self.escaped.to_string(),
            self.hostages_killed.to_string(),
            self.harvest_handovers.to_string(),
            self.captives_unfed.to_string(),
            self.lev_hostage_tries.to_string(),
            self.lev_hostage_ok.to_string(),
            self.k_posted[0].to_string(),
            self.k_done[0].to_string(),
            self.k_posted[1].to_string(),
            self.k_done[1].to_string(),
            self.k_posted[2].to_string(),
            self.k_done[2].to_string(),
            self.k_posted[3].to_string(),
            self.k_done[3].to_string(),
            self.k_posted[4].to_string(),
            self.k_done[4].to_string(),
            self.k_posted[5].to_string(),
            self.k_done[5].to_string(),
            self.fabricated.to_string(),
            self.exposed.to_string(),
            self.steal_runs.to_string(),
            self.flow_ransom.to_string(),
        ]
        .join(",")
    }
}

/// M16b phase 3: the wounded's CSV columns, after the captives'.
/// `down_now` and `trauma_subs` are day-end snapshots; the rest daily
/// counters. `downed` counts wounds (a fight's death made a wound),
/// `wounds_capped` the would-be wounds that stayed deaths at the quota,
/// `finished_off` the downed killed where they lay, `rescued_*` the
/// stabilisations by the carrier's tie, `stat_wounds` off-screen killings
/// survived as assaults (the Statistical split).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WoundedCols {
    pub downed: u32,
    pub wounds_capped: u32,
    pub bled_out: u32,
    pub finished_off: u32,
    pub stabilised: u32,
    pub recovered: u32,
    pub rescued_kin: u32,
    pub rescued_gang: u32,
    pub rescued_colleague: u32,
    pub rescued_guard: u32,
    pub trauma_calls: u32,
    pub trauma_saves: u32,
    pub down_now: u32,
    pub trauma_subs: u32,
    pub trauma_subs_new: u32,
    pub trauma_lapsed: u32,
    pub stat_wounds: u32,
    pub flow_trauma: i64,
}

impl WoundedCols {
    pub fn is_zero(&self) -> bool {
        *self == WoundedCols::default()
    }

    pub fn header() -> String {
        [
            "downed",
            "wounds_capped",
            "bled_out",
            "finished_off",
            "stabilised",
            "recovered",
            "rescued_kin",
            "rescued_gang",
            "rescued_colleague",
            "rescued_guard",
            "trauma_calls",
            "trauma_saves",
            "down_now",
            "trauma_subs",
            "trauma_subs_new",
            "trauma_lapsed",
            "stat_wounds",
            "flow_trauma",
        ]
        .join(",")
    }

    pub fn csv(&self) -> String {
        [
            self.downed.to_string(),
            self.wounds_capped.to_string(),
            self.bled_out.to_string(),
            self.finished_off.to_string(),
            self.stabilised.to_string(),
            self.recovered.to_string(),
            self.rescued_kin.to_string(),
            self.rescued_gang.to_string(),
            self.rescued_colleague.to_string(),
            self.rescued_guard.to_string(),
            self.trauma_calls.to_string(),
            self.trauma_saves.to_string(),
            self.down_now.to_string(),
            self.trauma_subs.to_string(),
            self.trauma_subs_new.to_string(),
            self.trauma_lapsed.to_string(),
            self.stat_wounds.to_string(),
            self.flow_trauma.to_string(),
        ]
        .join(",")
    }
}

/// M16b phase 4: boards, tags, the Patrol and attention, after the
/// wounded's. `boards` the board corps, `campaign_moves` a challenger's
/// tries on a swing member, `board_persuades` the campaign's `Persuade`
/// records; `distracted` the factions below their pool at the snapshot,
/// `alert_law` the law's alertness ÷ pool, `alert_min` the lowest.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GovCols {
    pub votes: u32,
    pub votes_swung: u32,
    pub chairs_fallen: u32,
    pub boards: u32,
    pub campaign_moves: u32,
    pub board_persuades: u32,
    pub k_vote_posted: u32,
    pub k_vote_done: u32,
    pub k_tag_posted: u32,
    pub k_tag_done: u32,
    pub tags_planted: u32,
    pub tags_live: u32,
    pub tags_found: u32,
    pub scans: u32,
    pub patrols: u32,
    pub patrol_finds: u32,
    pub scanners_bought: u32,
    pub distracted: u32,
    pub alert_law: f32,
    pub alert_min: f32,
}

impl GovCols {
    pub fn is_zero(&self) -> bool {
        *self == GovCols::default()
    }

    pub fn header() -> String {
        [
            "votes",
            "votes_swung",
            "chairs_fallen",
            "boards",
            "campaign_moves",
            "board_persuades",
            "k_vote_posted",
            "k_vote_done",
            "k_tag_posted",
            "k_tag_done",
            "tags_planted",
            "tags_live",
            "tags_found",
            "scans",
            "patrols",
            "patrol_finds",
            "scanners_bought",
            "distracted",
            "alert_law",
            "alert_min",
        ]
        .join(",")
    }

    pub fn csv(&self) -> String {
        [
            self.votes.to_string(),
            self.votes_swung.to_string(),
            self.chairs_fallen.to_string(),
            self.boards.to_string(),
            self.campaign_moves.to_string(),
            self.board_persuades.to_string(),
            self.k_vote_posted.to_string(),
            self.k_vote_done.to_string(),
            self.k_tag_posted.to_string(),
            self.k_tag_done.to_string(),
            self.tags_planted.to_string(),
            self.tags_live.to_string(),
            self.tags_found.to_string(),
            self.scans.to_string(),
            self.patrols.to_string(),
            self.patrol_finds.to_string(),
            self.scanners_bought.to_string(),
            self.distracted.to_string(),
            format!("{:.3}", self.alert_law),
            format!("{:.3}", self.alert_min),
        ]
        .join(",")
    }
}

/// Transit (docs/TRANSIT.md § 4): the El's day. `commute_ride_*` are the
/// commutes that rode (ticks and Manhattan tiles door to door), read as
/// `commute_tpt_ride` beside `commute_tpt_walk`/`_drive`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransitCols {
    pub rides_full: u32,
    pub rides_coarse: u32,
    /// Phase 2 (T13): Statistical commute rides (two a riding workday).
    pub rides_stat: u32,
    /// Coins of fare paid (`Flow::Fare`).
    pub fares: i64,
    pub fare_dodges: u32,
    pub refused_no_route: u32,
    pub refused_fare: u32,
    pub refused_slow: u32,
    pub refused_danger: u32,
    /// A ride given up at the platform (no service, or the fare spent).
    pub ride_abandoned: u32,
    /// The most boardings at one stop in one hour today.
    pub peak_stop_load: u32,
    /// Coins of power paid by the Treasury to the World.
    pub transit_power: i64,
    /// Phase 3: the El's maintenance crews funded from fares (`TransitState.staff`).
    pub el_staff: u32,
    pub commute_ride_ticks: u64,
    pub commute_ride_tiles: u64,
    /// Phase 2, the parity test's counters (not columns): the bodies'
    /// commutes to work and those that rode; the Statistical workdays whose
    /// commute was judged (`transit::stat_commute`) and those that rode.
    pub commutes: u32,
    pub commute_rides: u32,
    pub stat_commutes: u32,
    pub stat_commute_rides: u32,
    /// Parity (arrests): the Statistical non-commute rides
    /// (`transit::stat_errand`; not a column).
    pub stat_errand_rides: u32,
}

impl TransitCols {
    pub fn is_zero(&self) -> bool {
        *self == TransitCols::default()
    }

    /// The header's names, comma-separated, in column order.
    pub fn header() -> String {
        [
            "rides_full",
            "rides_coarse",
            "rides_stat",
            "fares",
            "fare_dodges",
            "refused_no_route",
            "refused_fare",
            "refused_slow",
            "refused_danger",
            "ride_abandoned",
            "peak_stop_load",
            "transit_power",
            "el_staff",
            "commute_tpt_ride",
        ]
        .join(",")
    }

    /// The columns, comma-separated, in header order.
    pub fn csv(&self) -> String {
        let tpt = if self.commute_ride_tiles > 0 {
            self.commute_ride_ticks as f32 / self.commute_ride_tiles as f32
        } else {
            0.0
        };
        [
            self.rides_full.to_string(),
            self.rides_coarse.to_string(),
            self.rides_stat.to_string(),
            self.fares.to_string(),
            self.fare_dodges.to_string(),
            self.refused_no_route.to_string(),
            self.refused_fare.to_string(),
            self.refused_slow.to_string(),
            self.refused_danger.to_string(),
            self.ride_abandoned.to_string(),
            self.peak_stop_load.to_string(),
            self.transit_power.to_string(),
            self.el_staff.to_string(),
            format!("{tpt:.2}"),
        ]
        .join(",")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DailyStats {
    /// The day in progress.
    pub current: DayRow,
    /// Finished days, oldest first, capped at [`STATS_HISTORY_CAP`].
    pub history: VecDeque<DayRow>,
}

impl DailyStats {
    pub fn new() -> Self {
        DailyStats { current: DayRow::new(0), history: VecDeque::new() }
    }

    /// Close the current day: move it into history and start the next.
    pub fn roll(&mut self, next_day: u64) -> &DayRow {
        let finished = std::mem::replace(&mut self.current, DayRow::new(next_day));
        if self.history.len() >= STATS_HISTORY_CAP {
            self.history.pop_front();
        }
        self.history.push_back(finished);
        self.history.back().expect("just pushed")
    }
}

impl Default for DailyStats {
    fn default() -> Self {
        DailyStats::new()
    }
}
