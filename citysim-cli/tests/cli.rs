//! M0: `run --days 1 --seed 1 --report` prints the exact header line (M15 W43: the word's columns;
//! Jobs & lives phase 1: the `[jobs2]` group before `ticks_per_sec`, the switch on as shipped; Lives at work
//! phase 7 appends its nine columns there; the pre-view pass phase 2 its nine `shift_*` after them).

use std::process::Command;

fn cli() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_citysim-cli"));
    c.current_dir(env!("CARGO_MANIFEST_DIR"));
    c
}

#[test]
fn test_cli_report_csv_header() {
    let out = cli().args(["run", "--days", "1", "--seed", "1", "--report"]).output().expect("run cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next(),
        Some("day,season,population,employed,homeless,jailed,gang_members,food_market,food_warehouse,food_pantry,price,treasury,thefts,arrests,deaths_starvation,deaths_old_age,deaths_violence,births,immigrants,emigrants,burials,mean_hunger,mean_mood,goal_changes_per_agent,holes_opened,holes_open,holes_bound,holes_unknown,deaths_violence_offscreen,tier_full,tier_coarse,tier_stat,evictions,rent_paid,rent_short,housed,flow_food,flow_drink,flow_wages,flow_rent,flow_upkeep,flow_wholesale,flow_overflow,flow_restock,flow_contract,flow_tax,flow_dole,flow_other,wallets,wallet_gini,wallet_top10,corp1_treasury,corp1_order,corp2_treasury,corp2_order,corp3_treasury,corp3_order,corp4_treasury,corp4_order,corp5_treasury,corp5_order,corp6_treasury,corp6_order,corp7_treasury,corp7_order,corp8_treasury,corp8_order,corp9_treasury,corp9_order,acquisitions,bankruptcies,monopolies,foundings,incorporations,strikes,unrest_corp,unrest_street,unrest_dreg,class_corp,class_street,class_dreg,happiness_street,d1_coverage,d1_control,d1_litter,d1_unrest,d1_crime,d1_guards,d2_coverage,d2_control,d2_litter,d2_unrest,d2_crime,d2_guards,d3_coverage,d3_control,d3_litter,d3_unrest,d3_crime,d3_guards,d4_coverage,d4_control,d4_litter,d4_unrest,d4_crime,d4_guards,d5_coverage,d5_control,d5_litter,d5_unrest,d5_crime,d5_guards,d6_coverage,d6_control,d6_litter,d6_unrest,d6_crime,d6_guards,d7_coverage,d7_control,d7_litter,d7_unrest,d7_crime,d7_guards,d8_coverage,d8_control,d8_litter,d8_unrest,d8_crime,d8_guards,hotel_nights,squatters,derelicts,vagrancy,riots,crossfire,gangs,vehicles_moto,vehicles_car,vehicles_truck,vehicles_flyer,truck_hauls,walk_hauls,commute_tpt_walk,commute_tpt_drive,chrome_installs,chrome_agents,mean_sanity,episodes,hooked,stims_dealt,stims_legal,dealing_reports,repos,impounds,crashes,crash_deaths,vehicle_thefts,chops,abductions,stripped,robots,flow_asset,flow_asset_upkeep,flow_finance,flow_import,flow_stims,flow_parts,flow_treatment,overdoses,harvests,stripped_window,gang_income,gang_income_dealing,episodes_by_law,treatments,detoxes,nodes,labs,decks,runs,runs_ok,data_made,data_stolen,data_wiped,data_sold,ledger_hacks,doors_hacked,traced,fried,flatlined,hack_arrests,ice_mean_corp,ice_spend,runs_bounced,runs_captured,runs_dumped,hack_arrests_chair,robots_turned,blinded,cameras,sightings,ice_raised,ice_lowered,tech_gained,tech_lost,research_spent,data_held,corp1_tier_chrome,corp1_tier_deck,corp1_tier_industry,corp1_data,corp2_tier_chrome,corp2_tier_deck,corp2_tier_industry,corp2_data,corp3_tier_chrome,corp3_tier_deck,corp3_tier_industry,corp3_data,corp4_tier_chrome,corp4_tier_deck,corp4_tier_industry,corp4_data,corp5_tier_chrome,corp5_tier_deck,corp5_tier_industry,corp5_data,corp6_tier_chrome,corp6_tier_deck,corp6_tier_industry,corp6_data,corp7_tier_chrome,corp7_tier_deck,corp7_tier_industry,corp7_data,corp8_tier_chrome,corp8_tier_deck,corp8_tier_industry,corp8_data,corp9_tier_chrome,corp9_tier_deck,corp9_tier_industry,corp9_data,flow_data,flow_hack,flow_ice_upkeep,flow_research,flow_terminal,rumours_heard,second_hand_share,known_by_killers_median,distorted,grudges,grudges_inherited,hunts,hunts_active,avenged,revenge_kills,chain_max,vendettas_open,stories,planted,buried,poached,talent_lost,skill_rare_share,extort_success,rep_flips,rumour_hops_max,pool_reach,contradicted,silenced,hunts_failed,hunts_abandoned,guard_body,expelled,contracts_lost_honour,extort_tries,g1_dread,g1_heat,g2_dread,g2_heat,g3_dread,g3_heat,g4_dread,g4_heat,c1_honour,c1_standing,c1_competence,c2_honour,c2_standing,c2_competence,c3_honour,c3_standing,c3_competence,c4_honour,c4_standing,c4_competence,c5_honour,c5_standing,c5_competence,c6_honour,c6_standing,c6_competence,c7_honour,c7_standing,c7_competence,c8_honour,c8_standing,c8_competence,c9_honour,c9_standing,c9_competence,law_competence,flow_ads,flow_plant,flow_leisure,flow_gamble,flow_gamble_win,flow_tribute,flow_export,flow_public_works,wage_dole_ratio,employed_share,venues_club,venues_arcade,venues_noodle_bar,venues_fight_pit,venues_den,venues_lounge,visits_club,visits_arcade,visits_noodle_bar,visits_fight_pit,visits_den,visits_lounge,fab_parts,scrap_parts,parts_imported,works_jobs,upkeep_mult,outside_inbound,outside_minted,fun_mean,fun_satisfied_share,hangouts,hangout_contacts_mean,fronts,collected,preached,d1_street_density,d2_street_density,d3_street_density,d4_street_density,d5_street_density,d6_street_density,d7_street_density,d8_street_density,fv_killed,fv_assaulted,fv_robbed,fv_abducted,fv_bound,fv_unknown,fv_capped,fv_bound_wrong,kill_rate_body,kill_rate_stat,kill_rate_body_civ,kill_rate_stat_civ,tier_held,tier_held_body,held_fed,gang_bodies,gang_stat,stat_extorts,stat_claims,stat_deals,aborts,aborts_scavenge,aborts_sleep,aborts_checkin,aborts_seat,rough_sleeps,scavenge_dry,flow_street_dice,contracts_open,contracts_posted,contracts_fulfilled,contracts_failed,contracts_expired,contracts_cancelled,reneged,k_hit_posted,k_hit_done,k_beat_posted,k_beat_done,k_guard_posted,k_guard_done,k_locate_posted,k_locate_done,hits_done,hits_squad,hits_solo_weak,strikes_declined_pol,sold_out,contract_murders,contract_cleared,contract_holes,contract_hole_wrong,accessory,interrogations,interrogations_won,bounties_paid,guards_on_take,fixer_runs,regulars,live_parties,live_queued,escrow_held,escrow_leak,escrow_stuck,f0_heat,f1_heat,f2_heat,f3_heat,f0_income,f1_income,f2_income,f3_income,flow_escrow,flow_payout,flow_fixer_cut,contracts_refused_full,flow_import_out,flow_customs,flow_inputs,flow_property,flow_donate,trade_balance,trade_balance_30,appetite_food,appetite_parts,appetite_data,bid_food,ask_food,food_imported,food_exported,wage_mult_mean,wage_gross_mean,vacancies_open,laid_off,pop_inflow_wages,pop_inflow_other,tax_rate,mission_meals,mission_cots,mission_purse,flow_migrant_in,flow_migrant_out,flow_fence,flow_alms,flow_camp_food,outside_outbound,world_treasury,coin_identity,parts_exported,data_exported,export_paid,grow_world,city_owned_buildings,recycler_till,children_taken,camp_children,camp_output,camp_unfed,camp_released,camp_scandals,d1_meals,d1_starving,d1_dregs,d2_meals,d2_starving,d2_dregs,d3_meals,d3_starving,d3_dregs,d4_meals,d4_starving,d4_dregs,d5_meals,d5_starving,d5_dregs,d6_meals,d6_starving,d6_dregs,d7_meals,d7_starving,d7_dregs,d8_meals,d8_starving,d8_dregs,civic_payroll,flow_fines,guard_count,sanitation_count,migrants_pull,migrants_offer,trades_employed,super_rounds,tax_credit,meals_real,meals_synth,meals_dive,meals_alms,malnourished,toxic,mean_malnutrition,mean_toxicity,sick_days,cancer_new,cancer_diagnosed,cancer_cured,deaths_disease,bin_meals_in,bin_contests,bin_shakedowns,hm_posted_revenge,hm_posted_rival,hm_posted_culprit,hm_posted_silence,hm_posted_debt,hm_posted_protect,hm_wants,hm_unaffordable,hm_refused,hm_pooled,hm_price_sum,hm_deposit_sum,hm_hits_done,hm_beats_done,hm_stiffed,hm_recanted,hm_cold,hm_commissions,coercions_active,lev_threat_tries,lev_threat_ok,lev_secret_tries,lev_secret_ok,lev_debt_tries,lev_debt_ok,coercions_broken,protection_made,protection_paid,protection_coins,protection_active,blackmail_made,blackmail_active,secrets_lapsed,silenced_threat,coerced_jobs,coerced_done,poach_threats,parleys,parley_fails,tribute_active,fake_jobs,frauds_filed,k_persuade_posted,k_persuade_done,flow_coerced,captives_held,prison_pop,prisons,hostages_taken,hostages_freed,ransomed,rescued_captives,escaped,hostages_killed,harvest_handovers,captives_unfed,lev_hostage_tries,lev_hostage_ok,k_abduct_posted,k_abduct_done,k_rescue_posted,k_rescue_done,k_escort_posted,k_escort_done,k_steal_posted,k_steal_done,k_deliver_posted,k_deliver_done,k_spin_posted,k_spin_done,fabricated,exposed,steal_runs,flow_ransom,downed,wounds_capped,bled_out,finished_off,stabilised,recovered,rescued_kin,rescued_gang,rescued_colleague,rescued_guard,trauma_calls,trauma_saves,down_now,trauma_subs,trauma_subs_new,trauma_lapsed,stat_wounds,flow_trauma,votes,votes_swung,chairs_fallen,boards,campaign_moves,board_persuades,k_vote_posted,k_vote_done,k_tag_posted,k_tag_done,tags_planted,tags_live,tags_found,scans,patrols,patrol_finds,scanners_bought,distracted,alert_law,alert_min,rides_full,rides_coarse,rides_stat,fares,fare_dodges,refused_no_route,refused_fare,refused_slow,refused_danger,ride_abandoned,peak_stop_load,transit_power,el_staff,commute_tpt_ride,outside_remit,outside_tariff,outside_parent_fund,outside_subsidies,outside_in,outside_out,outside_net,parents_failing,outsiders_living,outsiders_arrived,outsiders_left,settled,reinforcements,buybacks,retakes_posted,retakes_won,branches_founded,parents_founded,abandoned,brothers_arrived,gangs_refounded,state_guards,state_grants,mandates,withdrawals,uplink_runs,uplink_breached,data_shipped,sanctions_new,sanctions_active,campaigns,parents_dead,scrip_out,scrip_paid,scrip_spent,scrip_converted,scrip_wiped,parents_in,parents_out,successions,civil_wars,wars_active,war_loser_dead,war_loser_fled,war_loser_splinter,war_loser_jailed,abroad_posted,abroad_done,abroad_won,abroad_dead,away,promoted_cast,cast_arrived,blackwall_breaches,o1_treasury,o1_income,o1_health,o1_attitude,o1_rep,o1_loss,o1_order,o1_edge,o2_treasury,o2_income,o2_health,o2_attitude,o2_rep,o2_loss,o2_order,o2_edge,o3_treasury,o3_income,o3_health,o3_attitude,o3_rep,o3_loss,o3_order,o3_edge,o4_treasury,o4_income,o4_health,o4_attitude,o4_rep,o4_loss,o4_order,o4_edge,o5_treasury,o5_income,o5_health,o5_attitude,o5_rep,o5_loss,o5_order,o5_edge,flow_handling,handling_leak,flow_power,power_fuel,flow_build,food_packed,rations_made,rations_sold,rations_paid,pilfered,builds_started,builds_done,sites_open,dark_districts,outages,jailers,clerks,depot_hands,civic3_target,office_workers,hqs,emp_produce,emp_service,emp_handling,emp_contract,emp_civic,emp_overhead,emp_resale,emp_day,emp_packing,emp_freight,emp_utility,emp_construction,emp_hq,n_packing,n_freight,n_utility,n_construction,pitches,pitches_vendor,pitches_rickshaw,pitches_tinker,pitch_opened,pitch_closed,street_cut,swept,vendor_meals,vendor_take,day_posts_offered,day_posts_filled,day_posts_ended,corner_queue,day_labourers,parcels,parcel_fees,cab_trips,cab_fares,rickshaw_trips,rickshaw_fares,tinker_repairs,tinker_take,tinker_botched,plants_seized,weapons,carriers,armoury,precinct_arms,gunsmith_stock,licences,weapon_wants,weapons_made,sold_legal,sold_dealer,issued,seized,crates_landed,crates_seized,fights,fight_deaths,armed_fights,armed_deaths,ua_reports,licences_sold,pickpockets,stall_thefts,home_thefts,fenced,flow_licence,flow_arms,vice_venues,strip_visits,brothel_visits,companion_visits,vice_revenue,tips,street_pitches,street_visits,street_take,pimps,pimp_cut,street_assaults,solicitation,solicit_arrests,brothel_closed,boutiques,boutique_visits,clothes_sold,clothing,clothes_worn,clothes_stolen,status_corp,status_street,status_dreg,dollhouses,dollhouse_visits,chip_hired,chip_ended,chip_early,chip_contracts,chipped_living,chip_episodes,captives_chipped,trafficking,bd_studios,bd_made,bd_wholesale,bd_viewed,bd_exported,bd_export_paid,recorders,recorded,black_made,black_sold,black_revenue,patrol_sweeps,papers_held,fake_ids,fake_sold,papers_bribed,exposures,organs_made,organs_rotted,organs_to_clinics,organs_used,organs_exported,organs_export_paid,organs_seized,organ_stock,scav_gangs,scav_kills,black_clinics,bm_reports,struck_off,followers,followers_c1,followers_c2,followers_c3,followers_c4,cult_members,tithes,conversions,preach_attend,apostates,apostates_hunted,church_purse,ch_play,ch_school,ch_chores,ch_street,ch_camp,child_days,school_days,school_fees,street_takes,child_begged,child_thefts,schools,elders,elders_employed,elders_taken_in,elders_on_cots,maimed,maimed_new,prosthetics,served_visit,served_patient,served_pupil,served_inmate,served_tenant,served_fare,served_parcel,served_crate,served_client,served_door,served_staff,served_buyer,served_attendee,served_source,served_broker,served_waited,serve_shifts,cooked,staff_meals,regulars_made,door_refusals,ejections,flow_madam_cut,home_hangouts,friend_hangouts,home_with_spouse,empty_leaves,home_empty_leaves,reunited,kin_visits,elder_visits,carers,camp_beatings,camp_escapes,camp_headcounts,camp_visits,camp_buyouts,camp_breakouts,camp_neglect,notices,door_rent,super_stops,tied_housed,flow_buyout,taught_days,cab_rides_driven,cab_rides_unmanned,fares_short,parcels_delivered,parcels_returned,lifts,pilfer_reports,street_clients_near,pimp_collections,pimp_pass_up,corner_deals,dealer_regulars,lookout_halved,meetings_held,meeting_attendees,decisions,beat_stops,interviews,bylines,byline_promotions,walk_ins_served,walk_ins_waited,walk_ins_dropped,contract_encounters,fixer_collections,licences_named,bribes_named,band_refusals,foremen_premium,profession_rehires,foreman_hires,retirements,frail_exits,unretired,lieutenants,delegated_orders,lieutenant_raids,leader_cut_coins,lieutenant_cut_coins,stations,station_bookings,station_fines,station_nights,transfers,stations_full,precinct_full,execs,execs_spire,exec_rides,exec_rides_corp,shift_stations,shift_tiles,shift_breaks,shift_break_meals,shift_block_max,shift_long_60,shift_long_90,shift_long_180,shift_long_over,haul_shifts,haul_trips,haul_units,haul_away,parcels_missed,counter_shifts,counter_queued,served_day,served_21h,presence_missed,sweep_stops,sweep_rounds,sweep_stands,arms_meet,arms_midnight,arms_missed,cut_at_meet,sight_asleep,papers_handover,leg_segments,escort_walks,escort_vans,escort_tpm_max,exec_rides_home,fares_driven,fares_unmanned,fares_no_body,fares_too_far,lead_rounds,lead_stops,lead_collected,lead_owner_out,courts,court_handovers,orders_court,orders_walked,orders_word,company_beds,company_housed,company_moved_in,company_evicted,company_inside_beds,company_buyouts,company_built,company_notices,company_buyout_evicted,pressure_far,pressure_moved,pressure_company,pressure_movers,pressure_jobs,ticks_per_sec")
    );
    let row = lines.next().expect("one data row");
    // M10: 2,000 residents, 220 jobs (160 / 36 / 12 / 8 / 4), nobody homeless;
    // M11: plus 12 private guards hired at tick 0 (two Security Offices) and
    // a bartender for the agent-owned Bar short of its staff of 3.
    // (Off-screen thieves can be jailed on day 0 since M10 phase 3, and an
    // off-screen killing can take a resident, and their job, on day 0.)
    // M12 D26: the seeded derelicts' residents start homeless. M12 D23: the
    // first midnight hires 5 sweepers (the reconcile's daily cap): 220 + 12 +
    // 1 + 5 = 238 on seeds 1-3, less a resident jailed or killed off-screen
    // (fix pass: the bound 250 tightened to 230-240). M13 D18: plus up to
    // six Mechanics at the two seeded Garages, and (phase 3) up to six
    // Ripperdocs at the three seeded Clinics (230-252). M13 phase 5: the
    // Tech corp's day-0 Garage hires three more Mechanics (9 in all on seed
    // 1), and with nobody jailed or killed off screen that day seed 1 read
    // 253: 230-255. M14 V16: plus the four seeded Labs' sixteen Researchers
    // hired at the first midnight: 230-271. M15 W41: plus the two seeded
    // Feeds' Reporters (one each since phase 4; the bound kept at 230-277).
    // L2 phase 1: plus the seeded venues' and Fabs' 115 staff and the day-0
    // Registers' NoodleBar cooks (389 on seeds 1-3): 330-420. Jobs and room P3: plus the seeded trade
    // places hired at the first midnight (448 on this seed): 330-500.
    let cols: Vec<&str> = row.split(',').collect();
    assert_eq!(&cols[..2], ["0", "Spring"], "row: {row}");
    let n = |i: usize| cols[i].parse::<u32>().expect("a count");
    let derelict_homeless = 5 * citysim::Config::load().street.seed_derelict_blocks as u32;
    // The flip (2026-10-10, wages from revenue, no net): the transition
    // wave's seed_staff hires the city's staffing before day 0 (1,142 on seed
    // 1), and the seeded homeless are the derelicts' residents plus the
    // working poor housed nowhere near work (80 on seed 1): 1,000-1,300
    // employed, the derelicts' to 120 homeless. The dole city's row
    // (`Config::dole_city`) kept 330-500 and the derelicts' alone.
    // Workplace room (2026-10-11): the seeding stops at each workplace's
    // room on its shifts (the x3 ceiling had 44 Vat Techs in a Farm of 16):
    // 863 on seed 1, so 800-1,300.
    assert!((1995..=2000).contains(&n(2)) && (800..=1300).contains(&n(3)), "row: {row}");
    // Lives at work phase 6: the first midnight retires 32 elders to their
    // children's Homes, some the derelicts' residents (74 on seed 1, 78 with
    // careers off): the floor 5 lower since the switch retired (OPEN-094).
    assert!((derelict_homeless - 10..=120).contains(&n(4)), "row: {row}");
    // M13 D49: corp slot 9 and the 40 asset columns; M14 V43: the 72 Virt
    // columns; M15 W43: the 68 word columns (30, 4 gangs x 2, 9 corps x 3, 3);
    // L2 (plan L34): the 54 living-city columns; L2 phase 3: the 15 budget columns;
    // the review fix's `flow_street_dice`; M16a (plan C38): the 48 contract columns
    // (49 with phase 3's `contracts_refused_full`; 48 since the M16a review
    // retired `accessory_unfounded`);
    // the Real economy (plan E45): the 68 econ columns; Jobs v2 P5a (J34): the
    // 4 jobs columns; Jobs v2 P6: `migrants_pull`, `migrants_offer`; Jobs v2 P3: `trades_employed`, `super_rounds`; the surplus credit: `tax_credit`;
    // hunger as sickness: the 16 health columns; the Hit market
    // (docs/HIT_MARKET.md § 7): the 18 `HitCols`; transit
    // (docs/TRANSIT.md § 4): the 14 transit columns; M16b phase 1: the 27
    // `LeverageCols`; phase 2: the 29 `CaptiveCols`; phase 3: the 18 `WoundedCols`; phase 4: the 20 `GovCols`;
    // M17 phase 2: the 43 `OutsideCols` (8 + 7 a parent x 5); phase 3: 17 more; phase 4: 12 more and an edge a parent;
    // phase 4b: the 7 succession columns; phase 5: the 8 Abroad and Blackwall columns.
    // Jobs & lives phase 1: the 38 `Jobs2Cols` (the switch on as shipped); phase 2: 25 street columns;
    // phase 3: 26 weapons columns; phase 4: 24 vice and fashion columns; phase 5: 22 chip, braindance
    // and patrol-sweep columns; phase 6: 30 gray-market and faith columns; phase 7: 19 life-stage
    // columns. Lives at work phase 1: the 23 encounter columns (15 served_<kind> and 8 counters); phase 2: 9
    // household columns; phase 3: 13 civic columns; phase 4: 13 street columns; phase 5: 14 tower columns;
    // phase 6: 12 careers and leaders columns; phase 7: 9 stations and Spire columns; gate 8d: `exec_rides`,
    // `exec_rides_corp`; the pre-view pass phase 2: 9 `shift_*` columns; phase 3: 9 haul and counter columns; phase 4: 10 presence columns; phase 5: 9 legs and fares columns;
    // phase 6: 9 leaders columns; the island pass phase 3a: 4 company housing columns; phase 3b: 5 more.
    assert_eq!(
        row.split(',').count(),
        81 + 8 * 6
            + 7
            + 2
            + 40
            + 72
            + 68
            + 54
            + 15
            + 1
            + 48
            + 68
            + 4
            + 2
            + 2
            + 1
            + 16
            + 18
            + 27
            + 29
            + 18
            + 20
            + 14
            + 43
            + 17
            + 19
            + 7
            + 8
            + 38
            + 25
            + 26
            + 24
            + 22
            + 30
            + 19
            + 23
            + 9
            + 13
            + 13
            + 14
            + 12
            + 9
            + 2
            + 9
            + 9
            + 10
            + 9
            + 9
            + 4
            + 5
    );
    assert!(lines.next().is_none());
}

#[test]
fn test_cli_ten_days_seed_42() {
    let out = cli().args(["run", "--days", "10", "--seed", "42", "--report"]).output().expect("run cli");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let rows: Vec<&str> = stdout.lines().skip(1).collect();
    assert_eq!(rows.len(), 10);
    // M13 D20/D47 and M14 V14: a crash, an overdose and a Flatline are deaths
    // outside the three `deaths_*` columns.
    let header: Vec<&str> = stdout.lines().next().expect("header").split(',').collect();
    let other: Vec<usize> = ["crash_deaths", "overdoses", "flatlined"]
        .iter()
        .map(|c| header.iter().position(|h| h == c).expect("a death column"))
        .collect();
    let outside_cols: Vec<usize> =
        ["outsiders_arrived", "brothers_arrived"].iter().filter_map(|c| header.iter().position(|h| h == c)).collect();
    // M10: at 2,000 residents a birth or a death in ten days is ordinary, so the population is
    // checked as an account: 2,000 + births + immigrants - deaths - emigrants, with at most
    // thirteen immigrants a week from day 7 (M11 § 7: the lever times a logistic of Street
    // happiness).
    let mut expected: i64 = 2000;
    let mut immigrants = 0;
    for (i, row) in rows.iter().enumerate() {
        let cols: Vec<i64> =
            row.split(',').enumerate().filter(|&(k, _)| k != 1).map(|(_, c)| c.parse().unwrap_or(0)).collect();
        // cols (season dropped): 0 day, 1 population, ..., 13..=15 deaths, 16 births, 17 immigrants, 18 emigrants
        assert_eq!(cols[0], i as i64);
        let raw: Vec<&str> = row.split(',').collect();
        let other_deaths: i64 = other.iter().map(|&k| raw[k].parse::<i64>().unwrap_or(0)).sum();
        // M17 phase 3: agents from abroad (Outsiders, brothers) arrive outside the immigrant count.
        let abroad: i64 = outside_cols.iter().map(|&k| raw[k].parse::<i64>().unwrap_or(0)).sum();
        expected += cols[16] + cols[17] + abroad - cols[13] - cols[14] - cols[15] - cols[18] - other_deaths;
        immigrants += cols[17];
        assert_eq!(cols[1], expected, "day {i}");
        assert!(immigrants <= 13 * (i as i64 / 7), "day {i}");
        if i % 7 != 0 {
            assert_eq!(cols[17], 0, "immigrants arrive weekly: day {i}");
        }
    }
}

#[test]
fn test_cli_lever_and_save_at() {
    let dir = std::env::temp_dir().join(format!("citysim-cli-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = cli()
        .args(["run", "--days", "1", "--seed", "3", "--report", "--events"])
        .args(["--lever", "day=0:release_reserve=200", "--save-at", "100"])
        .arg("--saves-dir")
        .arg(&dir)
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let row = stdout.lines().nth(1).expect("row");
    // M10: three Markets fill during the day and farms overflow into the Warehouse, so its stock
    // is dynamic now; the release itself is checked through its event (split over 3 Markets).
    let events = String::from_utf8_lossy(&out.stderr);
    assert!(events.contains("Released 200 food from the Reserve Depot to 3 Street Markets"), "no release event");
    assert!(dir.join("3-100.ron").is_file());

    // resume from the save and finish the day: identical row
    let out2 = cli()
        .args(["run", "--days", "1", "--seed", "3", "--report", "--load"])
        .arg(dir.join("3-100.ron"))
        .output()
        .expect("run cli");
    assert!(out2.status.success(), "stderr: {}", String::from_utf8_lossy(&out2.stderr));
    let stdout2 = String::from_utf8(out2.stdout).expect("utf8");
    let row2 = stdout2.lines().nth(1).expect("row");
    let strip = |r: &str| r.rsplit_once(',').map(|(a, _)| a.to_string()).expect("row");
    assert_eq!(strip(row2), strip(row));
    // replaying with the same --lever must not double-apply the lever already in the save
    let out3 = cli()
        .args(["run", "--days", "1", "--seed", "3", "--report", "--lever", "day=0:release_reserve=200", "--load"])
        .arg(dir.join("3-100.ron"))
        .output()
        .expect("run cli");
    assert!(out3.status.success());
    let stderr3 = String::from_utf8_lossy(&out3.stderr);
    assert!(stderr3.contains("skipped"), "stderr: {stderr3}");
    let stdout3 = String::from_utf8(out3.stdout).expect("utf8");
    assert_eq!(strip(stdout3.lines().nth(1).expect("row")), strip(row));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_cli_save_at_end_tick_and_out_of_range() {
    let dir = std::env::temp_dir().join(format!("citysim-cli-test-end-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = cli()
        .args(["run", "--days", "1", "--seed", "4", "--save-at", "1440", "--saves-dir"])
        .arg(&dir)
        .output()
        .expect("run cli");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("4-1440.ron").is_file(), "save at the final tick must be written");
    let out = cli()
        .args(["run", "--days", "1", "--seed", "4", "--save-at", "1441", "--saves-dir"])
        .arg(&dir)
        .output()
        .expect("run cli");
    assert_eq!(out.status.code(), Some(2), "out-of-range --save-at must fail loudly");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_cli_bad_lever_exits_2() {
    let out = cli().args(["run", "--days", "1", "--seed", "1", "--lever", "nope"]).output().expect("run cli");
    assert_eq!(out.status.code(), Some(2));
}

/// M14 phase 4: every new lever parses and the plain ones apply (the
/// index-based god levers resolve against a live world: an index that names
/// nothing is skipped with a warning, not a parse error).
#[test]
fn test_cli_virt_levers_parse_and_apply() {
    let levers = [
        "day=1:city_ice=3",
        "day=1:data_tax=0.2",
        "day=1:hack_sentence=intrusion:9",
        "day=1:hack_sentence=data_theft:20",
        "day=1:wipe_data=0",
        "day=1:set_tech=0:chrome:2",
        "day=1:grant_data=0:deck:10",
        "day=1:grant_deck=0:2",
        "day=1:set_ice=0:2",
        "day=1:fry=0",
        "day=1:run_now=0:0:data",
        "day=1:run_now=0:corp0:ledger",
    ];
    let mut c = cli();
    c.args(["run", "--days", "3", "--seed", "42", "--events"]);
    for l in levers {
        c.args(["--lever", l]);
    }
    let out = c.output().expect("run cli");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stderr: {err}");
    for want in [
        "City ICE set to 3",
        "Data tax set to 20 %",
        "Sentence for Intrusion set to 9 days",
        "Sentence for Data Theft set to 20 days",
    ] {
        assert!(err.contains(want), "missing {want:?} in the events:\n{err}");
    }
    assert!(!err.contains("unknown lever") && !err.contains("expected <"), "a lever failed to parse:\n{err}");
}
