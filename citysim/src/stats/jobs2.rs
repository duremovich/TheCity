//! Jobs & lives (docs/JOBS_AND_LIVES.md § 12): the milestone's group, only with `[jobs2] enabled`.

use super::{is_zero_i64, is_zero_u32};
use serde::{Deserialize, Serialize};

cols! {
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
        pub status_corp: f32 => fmt("{:.3}"),
        pub status_street: f32 => fmt("{:.3}"),
        pub status_dreg: f32 => fmt("{:.3}"),
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
        /// Round 6 V1 (OPEN-231): children out of a Camp on a warning older
        /// than `[camp] warn_stale_days` (a warning standing), at the day's end.
        #[serde(default)]
        pub ch_warned_old: u32,
        pub child_days: u32,
        pub school_days: u32,
        pub school_fees: i64,
        pub street_takes: u32,
        pub child_begged: i64,
        pub child_thefts: u32,
        /// Round 4 T2 (OPEN-232): a school-age child home on a school day: no
        /// School within `[ages] school_trip_ticks` (`school_far`), every one in
        /// reach full or with no teacher (`school_full`), the fee or the El's
        /// fares beyond the purse (`school_unpaid`); and the school days ridden
        /// on the El (`school_el`, the fares paid).
        pub school_far: u32,
        pub school_full: u32,
        pub school_unpaid: u32,
        pub school_el: u32,
        /// Round 5 U1 (OPEN-232): Schools founded on demand today (a district's
        /// children turned away full `[ages] found_full_days` running).
        pub schools_founded: u32,
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
        pub served: [u32; 15] => with(
            |h| h.extend(crate::components::ServeKind::ALL.iter().map(|k| format!("served_{}", k.label()))),
            |s, v, _| v.extend(s.served.iter().map(u32::to_string))
        ),
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
        /// Round 4 T1 (OPEN-231): the service's warnings at the door (one a
        /// child), the children let home, the absconders walked back in.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub camp_warnings: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub camp_let_home: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub camp_caught: u32,
        /// Round 5 U2 (OPEN-231): children gone to live with a free parent
        /// elsewhere (`camp::follow_parents`), takes on a second neglect inside
        /// `warn_repeat_days`, takes with no free parent at home.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub camp_followed: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub camp_take_repeat: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub camp_take_no_parent: u32,
        /// Round 6 V1 (OPEN-242): slipped-out children fetched back
        /// (`camp_fetched`); street children gone to live with a free parent
        /// with a Home (`camp_off_street`) or taken with no free parent or on
        /// an open warning (`camp_take_street`).
        #[serde(default)]
        pub camp_fetched: u32,
        #[serde(default)]
        pub camp_off_street: u32,
        #[serde(default)]
        pub camp_take_street: u32,
        /// Round 6 V1 (OPEN-231): food units a parent carried put on the shelf
        /// for his children's meal (`shelf::pocket_to_shelf`).
        #[serde(default)]
        pub camp_pocket: u32,
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
        /// Round 6 V4 (OPEN-239): the door's refusals of the drunk.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub door_drunk: u32,
        /// Round 6 V4 (OPEN-240): a rider's cab or rickshaw fares by payee: a
        /// driver (a fare trade's post or a rickshaw's puller), anyone else
        /// (the Garage's owner: a corp or a stranger).
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub cab_paid_driver: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub cab_paid_other: u32,
        /// Round 6 V4 (OPEN-240): the relocation's midnight read, workers whose
        /// Home-to-post commute is past `[life] commute_long_ticks`.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub commute_long: u32,
        /// Round 9 Y4: the same midnight read's median Home-to-post commute
        /// (`life::commute_cost`, travel minutes) over the workers it read.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub commute_med: u32,
        /// Round 6 V4 (OPEN-240): a retirement to kin passed over because the
        /// working spouse's commute from the child's Home would be too long.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub kin_spouse_far: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub shape_up_home: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub foreman_gate: u32,
        /// Round 4 T5 (OPEN-217): a body's search day begun (its Friends asked),
        /// a Friend's word of a post, a door asked, a hire at the door, and the
        /// midnight job search's far post (past `hire_max_ticks`) taken by a
        /// native who takes worse work.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub search_days: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub search_leads: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub search_asks: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub search_hires: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub far_hires: u32,
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
        /// Round 5 U5 (OPEN-217): forged weapons licences handed over at a
        /// Fixer's counter, passed and exposed at a guard's look (or an
        /// arrest), and brokered records closed with the office's fixer told.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub licences_forged: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub licences_passed: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub licences_exposed: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub fixer_outcomes: u32,
        /// Round 5 U6: clients a pimp's body sent to a dry pitch.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub pimp_booked: u32,
        /// Round 5 U6: off-screen installs booked for an hour of the clinic's day;
        /// kept at their hour; patients a doctor's body served at a Clinic.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub clinic_booked: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub clinic_kept: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub clinic_body: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub clinic_washes: u32,
        /// Round 7 W6 (OPEN-217): off-screen door refusals by cause (broke,
        /// barred, trouble: drunk, high, a brawler, carrying, a hot name; a
        /// rival's colours; the rest); bout purses paid (count, coins) and
        /// losers hurt; named pimp-days and those with a collection; Clinic-days
        /// with a Ripperdoc on staff at work and those with no patient.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub door_stat_broke: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub door_stat_barred: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub door_stat_trouble: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub door_stat_colours: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub door_stat_other: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub bout_purses: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub bout_purse_coins: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub bout_hurt: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub pimp_named_days: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub pimp_round_days: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub clinic_doc_days: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub clinic_idle_days: u32,
        // Round 8 X2 (OPEN-247): the mourners' shifts on the employer's leave
        // and past it, the funerals held; the target's answer to word of a Hit or
        // Beat (a guard on himself, a counter-grudge, lying low) and the shifts
        // he lay low; the search days of the needy past `[search] search_days`.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub grief_leave: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub grief_absent: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub grief_funerals: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub hit_guarded: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub hit_countered: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub hit_low: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub hit_low_shifts: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub search_need_days: u32,
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
        /// Round 6 V4 (OPEN-240): body rides handed to the off-screen cab when
        /// the driver's plan left the fare (`streetwork::carry`).
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub fares_dropped: u32,
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
        /// Round 7 W3 (`OPEN-244`): suspects with an open warrant at the day's
        /// first tick (a snapshot).
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub law_open_warrants: u32,
        /// Round 7 W3: warrants taken to the suspect's door, and the arrests made
        /// at home.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub law_door_taken: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub law_door_arrests: u32,
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
        // Round 5 U3 (OPEN-235, `systems::charity`): the soup line. A starving
        // adult served a bowl (both tiers); a body in the line waiting out the
        // hour's cap; the hungry poor turned away with a kitchen in reach and its
        // pot dry; the off-screen starving with no kitchen within reach.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub soup_starving_fed: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub soup_queued: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub soup_short: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub soup_far: u32,
        // Pressure pass round 6, V2 (`systems::turf`): the day's cells held (all
        // gangs), contested, and the gangs holding one (snapshots at the pass);
        // holder changes told, cells taken from a gang, holds contested; corner
        // holds planned and body hours on a corner; contested cells passed over
        // after dark (told), families moved out of one, recruits taken by the
        // holder of their cell, Street pitches opened in a held cell; the turf's
        // tribute (coins).
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_cells_held: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_cells_contested: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_gangs_holding: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_changes: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_lost: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_contests: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_corner_holds: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_corner_hours: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_avoided: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_family_moves: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_recruits: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_pitches: u32,
        #[serde(skip_serializing_if = "is_zero_i64")]
        pub turf_tribute: i64, // Round 6 V5 (TURF § 3, the war): provocations seen (heat added),
        // answers opened (the leader's muster, a front or a retake scheduled),
        // answers with nobody of the other side there, corner fights, cells
        // flipped by a fight, cells taken back by the side that lost them, cells
        // moved by a raid, wars started, members dead in corner fights, sides
        // exhausted (LieLow), truces, the law's crackdowns on a cell.
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_provocations: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_answers: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_unopposed: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_fights: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_flips: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_retaken: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_raid_moves: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_wars: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_war_deaths: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_exhausted: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_truces: u32,
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub turf_crackdowns: u32,
        /// Gang members killed (any cause of violence, any tier).
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub gang_deaths: u32,
        /// Round 7 W2 (`OPEN-241`): a corner fight's called men (the mover's bodies living about the cell) ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub war_called: u32,
        /// ... and of them, those at the corner when it was fought
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub war_came: u32,
        /// Round 7 W2: the gang members (not leaders) on a war line (the call, the fight, the war, lying low, the truce, the crackdown)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub war_member_lines: u32,
        /// Round 7 W2: a gang raid's marchers when it went in (on the march or at the door) ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub raid_marchers: u32,
        /// ... and of them, those at the door
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub raid_at_door: u32,
        /// Round 7 W2: marchers still on the way when their raid was over (the walk released)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub raid_missed: u32,
        /// Round 7 W2 (`OPEN-238`): a shakedown's payers asleep when it took their coins ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub sleeper_paid: u32,
        /// ... who woke to find it ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub sleeper_found: u32,
        /// ... and of those, reported to the law (a tell, a hue, a filed report) inside `[crime.case] tell_days`
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub sleeper_reports: u32,
        /// Round 7 W2: a sleeper robbed who woke and told nobody (his gang, the block's holders, not one for the law)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub sleeper_quiet: u32,
        /// Round 7 W2: a found body's finder who saw the murder ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub witness_finds: u32,
        /// ... and named the killer to the law ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub witness_named: u32,
        /// ... or kept quiet (the reason his line)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub witness_quiet: u32,
        /// Round 8 X1 (`OPEN-246`): a word promised to the law (a robbed sleeper woke, an awake payer: "means to tell") ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub word_meant: u32,
        /// ... kept (a report in his name: a desk, a guard on the street, a shout) ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub word_kept: u32,
        /// ... or dropped, the reason on his line (dealt with it himself, afraid, the law out of reach, taken in)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub word_dropped: u32,
        /// Round 8 X1: a witness recorded while held outside the scene's cells, or asleep (0 is right)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub witness_unfree: u32,
        /// Round 8 X1: an Attack across the cells' walls (one side held, the other not in the same cells; 0 is right)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub held_hit: u32,
        /// Round 8 X1: a blackmail or service visit credited to a visitor asleep (0 is right)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub asleep_acts: u32,
        /// Round 8 X1: a crime in the cells (the held on the held) given its case (the warden's report, days added)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub cell_cases: u32,
        /// Round 8 X4 (`OPEN-241`): a gang's leader changed (any way) ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub leader_changes: u32,
        /// ... and a Succession line told it to the members (here or `succession::decide`)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub leader_told: u32,
        /// Round 8 X4: a called member asleep near his corner roused for the fight
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub war_roused: u32,
        /// ... and of the mover's called at the corner when it was fought, those roused
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub war_roused_came: u32,
        /// Round 8 X4: a gang raid's members posted to its muster (within reach of it) ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub raid_posted: u32,
        /// ... and those who sat it out (the muster past `[crew] duty_reach_ticks` from home)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub raid_sat_out: u32,
        /// Round 8 X4: gang raids that left the muster (the first out the door) ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub raid_departed: u32,
        /// ... and of them, those whose column reached the door and went in or was called off there
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub raid_at_target: u32,
        /// Round 8 X4: a raid ordered: its fit members within reach of the muster picked ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub raid_reach_new: u32,
        /// ... and within reach of the old rule's (the held Home nearest the target)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub raid_reach_old: u32,
        /// Round 8 X6 (`OPEN-217`): a prisoner's whole day in the cells (counted at the midnight after) ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub cell_days: u32,
        /// ... and of them, the days a line named him (any event, `consequence::note_line`)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub cell_days_lined: u32,
        /// Round 8 X6: a prisoner's hour in the yard with nothing else on (a walk with a cellmate, or alone)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub cell_yard_walks: u32,
        /// V8 (`OPEN-256`): short terms bought out at booking from the purse
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub cells_fined: u32,
        /// V8: the coins of those fines
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub cells_fine_coins: u32,
        /// V8: prisoners bailed at midnight from their own or their kin's purse
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub cells_bailed: u32,
        /// V8: the coins of that bail
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub cells_bail_coins: u32,
        /// V8: prisoners let out early from a crowded Precinct
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub cells_early: u32,
        /// V8: a prisoner's morning on the work detail
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub cell_work: u32,
        /// V8: a hand of cards in the yard (two prisoners)
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub cell_cards: u32,
        /// V8: BreakOut orders dropped as chosen (too few within reach of the Precinct)
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub breakout_dropped: u32,
        /// V8: BreakOut marches that reached no Jail door
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub breakout_fizzled: u32,
        /// V8b (`OPEN-276`): a gang raid's muster held past its hour for its column, then left
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub raid_held: u32,
        /// V8b: a gang raid's muster broke up short of `raid_min_members` (or nobody came to it)
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub raid_broke: u32,
        /// Round 8 X6: a cells' meal served at `[justice] meal_hour` with its line
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub cell_meals: u32,
        /// Round 8 X6: a kin's (or Friend's) visit to a prisoner in the cells, credited
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub cell_visits: u32,
        /// Round 8 X6: a held witness of a cellmate's violent crime kept clear of him (no running in the cells)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub wr_held_clear: u32,
        /// Round 9 Y2 (`OPEN-250`): an arrest made by a guard on leave (mourning, lying low or his shift struck; 0 is right)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub leave_arrests: u32,
        /// Round 9 Y2: a gang's new leader who is in the cells as he takes the chair (0 is right)
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub cell_takeovers: u32,
        /// Round 9 Y2 (`OPEN-251`): a guard fought off by a suspect called a second guard (one answered) ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub backup_called: u32,
        /// ... and a resisting suspect faced more than one guard at the cuffs ...
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub backup_fights: u32,
        /// ... and of those, the suspect cuffed all the same
        #[serde(skip_serializing_if = "is_zero_u32")]
        pub backup_cuffed: u32,
        /// The combat model C3a (`combat::step`): live engagements ended today
        /// and their ticks (the mean length), suspects who fled the cuffs on
        /// foot, and bodies that broke and fled a fight on their legs (a flee plan)
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub live_fights: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub live_fight_ticks: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub arrest_escapes: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub fled_runs: u32,
        /// S2 (`OPEN-313`): a live fight's bystanders who ran off and who
        /// stopped to watch (`combat::stir`)
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub stir_fled: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub stir_watched: u32,
        /// The combat model C3b: joiners taken into fights (and raiders into a
        /// live raid); the lone body's duels against 2 and against 3 or more
        /// (and won); Sandevistan uses, Kerenzikov dodges, chrome limbs and
        /// robots wrecked in the beats; Hunts that gave chase to a fled target
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub joiners: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lone_1v2: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lone_1v2_won: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lone_1v3: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lone_1v3_won: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub sandy_uses: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub keren_dodges: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub chrome_wrecked: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub robots_wrecked: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub chases: u32,
        /// The city milestone C6b (`systems::floors`): the floor rent paid to
        /// the landlords today (`Flow::FloorRent`), the tenancies short on it,
        /// the floors taken back to a vacant unit and the leases passed to the
        /// landlord, the foundings into a unit, the company buyouts' leases of
        /// floors, and the vacant units at midnight.
        #[serde(default, skip_serializing_if = "is_zero_i64")]
        pub flow_floor_rent: i64,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub floor_short: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub floor_evicted: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub floor_taken: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub unit_foundings: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub floor_leases: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub units_vacant: u32,
        /// The city milestone C7 (`systems::levels`): the climbs inside a
        /// structure today (booked: a Full body's at the door, a Coarse one's at
        /// its walk's start) and their ticks, those to a storey of 20 or more
        /// and theirs, the lift rides among them and their waits, the longest
        /// lobby queue a call found; the walkway portals' hops today by a Full
        /// walker (a footbridge's own segments, the escalator's, the stairs,
        /// lifts, exits and platforms) and the hops from a tile onto a deck.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub climbs: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub climb_ticks: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub climbs_high: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub climb_ticks_high: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lift_rides: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lift_wait_ticks: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lift_queue_max: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub hops_footbridge: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub hops_escalator: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub hops_stairs: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub deck_entries: u32,
        /// The view milestone V7 (`signals`): Full walkers' steps onto a marked
        /// crossing, the minutes they waited at its kerb for the walk phase
        /// (tenths), their steps onto a carriageway off every crossing (a
        /// jaywalk begun), and the tiles they walked (the timed walks' crossing
        /// rate: crossings a tile).
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub crossings_walked: u32,
        /// V7: Full walkers' steps from the street up onto a footbridge (its riders).
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub footbridge_entries: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub crossing_wait_tenths: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub jaywalks: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub walk_steps: u32,
        /// The city milestone C8 (`systems::vertical`): district Hideouts
        /// founded today and standing at midnight, lifts a gang took hold of
        /// today and held at midnight, lifts cut today, raids that climbed to
        /// their target's floor (by the lift among them) and the bodies on the
        /// floors passed who heard one.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub hideouts_founded: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub hideouts_standing: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lift_holds: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lifts_held: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub lift_cuts: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub raid_climbs: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub raid_climb_lift: u32,
        /// V8 (`OPEN-264`): raiders whose own body climbed to the target's floor.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub raid_climb_bodies: u32,
        /// V8: founders who walked into a district Hideout founded on a floor.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub found_walkins: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub raid_heard: u32,
        /// C8: claims that took a held tower's upper floors (`vertical::split_claim`).
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub floor_claims: u32,
        /// The city milestone C9b (`systems::vertical`): the landlords' answers
        /// to a gang's unpaid floor rent today, by move; the gang Hideouts short
        /// on their floor rent at midnight (not swallowed); the gang raids landed
        /// today on a rival's seat and on one of its district Hideouts; the seat
        /// guards posted today.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub landlord_swallow: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub landlord_law: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub landlord_muscle: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub landlord_cut: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub landlord_sell: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub gang_rent_debts: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub raids_seat: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub raids_nearest: u32,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        pub seat_guards_posted: u32,
    }
}

impl Jobs2Cols {
    /// Lives at work phase 1: every encounter served today.
    pub fn served_total(&self) -> u32 {
        self.served.iter().sum()
    }
}

fn served_is_zero(v: &[u32; 15]) -> bool {
    v.iter().all(|&n| n == 0)
}
