use super::*;

/// Lives at work phase 2 `[household]` (docs/LIVES_AT_WORK.md § 3): the
/// home spot, the empty-spot leave, the household's rules and the visits.
/// Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HouseholdCfg {
    pub enabled: bool,
    /// § 3.1: a Home spot's score per household member home now, and per
    /// child at home.
    pub home_w: f32,
    pub child_w: f32,
    /// § 3.2: Friend/Family contacts' Homes offered as spots, at most.
    pub visit_hosts: u8,
    /// § 3.1: street dice between two bored co-hangers at home (off: none).
    pub home_dice: bool,
    /// § 3.3: a HangOut alone this long ends early; the spot scores ×
    /// `empty_penalty` for `empty_memory_hours`.
    pub empty_leave_ticks: u16,
    pub empty_penalty: f32,
    pub empty_memory_hours: u8,
    /// § 3.4: the newborn's days; the carer's away spots and night goals ×
    /// this; both parents' energy decay × this; the Home's pantry draw a
    /// day (Food, on the child food debt).
    pub newborn_days: u16,
    pub newborn_away_mult: f32,
    pub newborn_energy_mult: f32,
    pub newborn_cost_per_day: f32,
    /// § 3.4: the carer's night, hours of the day `[from, to)` (wrapping).
    pub night_from: u8,
    pub night_to: u8,
    /// § 3.4: a household business with no take for this many days is
    /// abandoned (its heir takes an outside post).
    pub abandon_days: u8,
    /// § 3.5: an elder parent unmet this long is visited; the visit's hour
    /// of the day `[from, to)`; the goal's flat bonus; the visitor's belonging.
    pub elder_visit_days: u8,
    pub visit_hour_from: u8,
    pub visit_hour_to: u8,
    pub visit_flat: f32,
    pub visit_belonging: f32,
    /// Round 8 X6 (`OPEN-217`): a kin or spouse in the cells unmet this
    /// many days is visited there (a `Hurt` visit to the Precinct or the
    /// Station: the day's one visit, one visitor a prisoner a day); 0 off
    /// (the default is the shipped value: `is_default` reads an off section).
    pub cell_visit_days: u8,
    /// § 3.5 (review fix 8, doctrine 3): the Visit goal's reason by purpose
    /// and its curve. Left out of a save at its defaults (the shipped values).
    #[serde(default, skip_serializing_if = "VisitUrgencyCfg::is_default")]
    pub visit_urgency: VisitUrgencyCfg,
}

/// `[household.visit_urgency]`: the Visit goal's "the reason" consideration,
/// by the visit's purpose, through `Linear { m: curve.0, b: curve.1 }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisitUrgencyCfg {
    /// A kin downed, assaulted or arrested.
    pub hurt: f32,
    /// An elder parent unmet for `elder_visit_days`.
    pub elder: f32,
    /// A child taken to a Camp (the parent's answer).
    pub camp: f32,
    /// The Fixer's walk-in pickup (phase 5).
    pub papers: f32,
    /// The Fixer owner's weekly collection (phase 5).
    pub collect: f32,
    /// The curve's `(m, b)`.
    pub curve: (f32, f32),
}

impl Default for VisitUrgencyCfg {
    fn default() -> Self {
        VisitUrgencyCfg { hurt: 1.0, elder: 0.6, camp: 0.8, papers: 0.9, collect: 0.5, curve: (0.5, 0.5) }
    }
}

impl VisitUrgencyCfg {
    pub fn is_default(&self) -> bool {
        *self == VisitUrgencyCfg::default()
    }
}

impl Default for HouseholdCfg {
    fn default() -> Self {
        HouseholdCfg {
            enabled: false,
            home_w: 0.5,
            child_w: 0.3,
            visit_hosts: 3,
            home_dice: false,
            empty_leave_ticks: 20,
            empty_penalty: 0.3,
            empty_memory_hours: 6,
            newborn_days: 30,
            newborn_away_mult: 0.3,
            newborn_energy_mult: 1.15,
            newborn_cost_per_day: 0.5,
            night_from: 21,
            night_to: 6,
            abandon_days: 7,
            elder_visit_days: 7,
            cell_visit_days: 3,
            visit_hour_from: 10,
            visit_hour_to: 20,
            visit_flat: 0.2,
            visit_belonging: 0.2,
            visit_urgency: VisitUrgencyCfg::default(),
        }
    }
}

impl HouseholdCfg {
    /// Not written into a save at its defaults (off).
    pub fn is_default(&self) -> bool {
        *self == HouseholdCfg::default()
    }
}

/// Lives at work `[lives]` (docs/LIVES_AT_WORK.md D13): the master switch.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LivesCfg {
    /// Off (the default when the table is absent): nothing of the milestone runs.
    pub enabled: bool,
}

impl LivesCfg {
    /// Not written into a save while off.
    pub fn is_off(&self) -> bool {
        !self.enabled
    }
}

/// Jobs & lives phase 7 `[ages]` (docs/JOBS_AND_LIVES.md § 6; D11-D13;
/// Dylan's answers 4 and 5): a child's daily routine (School on fees,
/// Chores at a parent's Pitch, Play, Street, Camp), the School, the
/// elderly's frailty and the kin who take them in, the seeded ages to
/// `seed_max_years`, and the maimed (a level and a slot; a prosthetic
/// clears it). Live only under `[jobs2] enabled`. Placeholders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgesCfg {
    pub enabled: bool,
    /// The seeded population's oldest age (sim years; `[world]
    /// age_max_years` with the switch off).
    pub seed_max_years: f32,
    /// Schools seeded (Mid and up, on the city's deed) per copy; its fee a
    /// school day paid by the household (`Flow::Contract`; the Treasury's
    /// with the `public_school` lever); `edu_gain` of education a school day
    /// at 18.
    pub schools: u32,
    /// Round 4 T2 (OPEN-232, Dylan 2026-10-27: reach by travel time): a
    /// child goes to the nearest School with room within `school_trip_ticks`
    /// of its Home door, on foot (`[exec] move_ticks_full` a tile) or by the
    /// El (`distance::travel`, when quicker), the household paying
    /// `school_el_fares` fares a day for the El; a School's roll holds
    /// `school_places_per_teacher` a teacher on its staff.
    pub school_trip_ticks: crate::time::Tick,
    pub school_el_fares: i64,
    pub school_places_per_teacher: u32,
    /// T2: each seeded School on the Lot whose door has the most Home
    /// residents not yet within the walk of `school_trip_ticks` of one
    /// (`false`: S6's district centroid, the probes pinned to that layout).
    pub school_by_reach: bool,
    /// Round 5 U1 (OPEN-232, Dylan 2026-10-28: Schools by the child count
    /// and on demand): once the children are seeded, the Schools are topped
    /// up to one for each `school_age_per_school` school-age children (the
    /// `schools` per copy the floor), each on the Lot (else a derelict
    /// Block) whose door has the most of them not yet given a seat within
    /// the walk of `school_trip_ticks` (0: no top-up, T2's per-copy seed).
    pub school_age_per_school: u32,
    /// U1: a district with `found_full_min` or more of its children turned
    /// away full on each of `found_full_days` school days running gets a
    /// School founded on the city's deed, by the same reach rule over the
    /// children turned away, staffed by the hire (0: none founded); not
    /// while a School in their reach still waits for its teachers.
    pub found_full_days: u32,
    pub found_full_min: u32,
    pub school_fee: i64,
    pub edu_gain: f32,
    /// A Street child begs `child_beg_coins` a day from a resident of its
    /// district (a gift), steals on `child_theft_p` (a resident's coins),
    /// and is taken to a camp after `street_take_days` on the street.
    pub child_beg_coins: i64,
    pub child_theft_p: f32,
    pub street_take_days: u16,
    /// The elderly: from `frail_from_years` work output × (1 − `frail_slope`
    /// × the years over); the sick-day firing at `elder_fire_mult` of
    /// `[sickness] sick_fire_days`; a jobless elder with an adult child
    /// housed elsewhere moves in on `kin_take_in_p` a day.
    pub frail_from_years: u32,
    pub frail_slope: f32,
    pub elder_fire_mult: f32,
    pub kin_take_in_p: f32,
    /// At seed a seeded adult of `kin_from_years` or more has a grown child
    /// (a Parent edge to an adult 20-35 years younger) on `seed_kin_p`.
    pub kin_from_years: u32,
    pub seed_kin_p: f32,
    /// The pressure pass S6 (OPEN-224): the seeded city's children. Each
    /// couple sharing a Home at the seed's end (and each couple an Ambient
    /// flat materialises) has `seed_child_slots` chances at a child, each on
    /// `seed_child_p` (a pure hash), aged uniformly under 18 and kept when
    /// the younger parent was `seed_child_gap_years[0]` to `[1]` years old at
    /// the birth; the child lives in the couple's Home as a born child does.
    pub seed_child_slots: u32,
    pub seed_child_p: f32,
    pub seed_child_gap_years: [f32; 2],
    /// S6: a child goes to School (the fee, the roster) from this age to 18;
    /// younger, it plays at home (or does chores at a parent's Pitch).
    pub school_from_years: u32,
    /// S6: a School child is out of its Home from `school_hours[0]` to
    /// `[1]` (minutes of the day) on a school day (`household::home_term`).
    pub school_hours: [u16; 2],
    /// S6: a Home's pantry keeps the children's food for this many days (a
    /// grocery run's level; a jobless parent shops for it by day).
    pub child_pantry_days: f32,
    /// S6 (Dylan 2026-10-27, a hard rule): nobody under this age is ever
    /// killed, hurt or harmed (`ages::is_protected`, read whatever `enabled`
    /// says; a child, `Child`, is protected at any setting).
    pub protected_under_years: u32,
    /// Disability: a stabilised survivor maimed on `p_maimed`; a packer's
    /// shift-hour on `p_press_maim`; a level's work output × (1 −
    /// `maimed_work`); a beggar's odds × `maimed_beg_mult`.
    pub p_maimed: f32,
    pub p_press_maim: f32,
    pub maimed_work: f32,
    pub maimed_beg_mult: f32,
}

impl Default for AgesCfg {
    fn default() -> Self {
        AgesCfg {
            enabled: false,
            seed_max_years: 75.0,
            schools: 2,
            school_trip_ticks: 60,
            school_el_fares: 2,
            school_places_per_teacher: 30,
            school_by_reach: true,
            school_age_per_school: 60,
            found_full_days: 5,
            found_full_min: 30,
            school_fee: 2,
            edu_gain: 0.003,
            child_beg_coins: 1,
            child_theft_p: 0.02,
            street_take_days: 14,
            frail_from_years: 60,
            frail_slope: 0.03,
            elder_fire_mult: 0.6,
            kin_take_in_p: 0.3,
            kin_from_years: 45,
            seed_kin_p: 0.6,
            seed_child_slots: 3,
            seed_child_p: 0.6,
            seed_child_gap_years: [18.0, 45.0],
            school_from_years: 6,
            school_hours: [540, 900],
            child_pantry_days: 2.0,
            protected_under_years: 18,
            p_maimed: 0.25,
            p_press_maim: 0.0005,
            maimed_work: 0.2,
            maimed_beg_mult: 1.5,
        }
    }
}

/// M17 phase 4b `[outside.succession]` (Dylan, 2026-10-13: "Gangs probably
/// should promote from within or even have a civil war to settle who the new
/// boss is"). A decapitated gang promotes the top of `leader_ranking`; when
/// the top two are within `war_margin` it fights a civil war for
/// `war_days`. Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SuccessionCfg {
    pub enabled: bool,
    /// The top two of `leader_ranking` closer than this contest the chair.
    pub war_margin: f32,
    /// A civil war needs this many free members (the claimants included).
    pub war_min_members: usize,
    /// The war is settled by the sides' strength after this many days.
    pub war_days: u32,
    /// Each claimant's grudge on the other (a Hunt; lethal at `[hunt] lethal_min`).
    pub war_grudge: f32,
    /// The strongest `war_seconds` of each side hold `war_grudge_second` on the rival claimant.
    pub war_seconds: usize,
    pub war_grudge_second: f32,
    /// Enemy edges across the sides at most (as D36's split).
    pub war_enemy_pairs: usize,
    /// A side's pull: `loyalty_w × loyalty` toward the heir (the ranking's top), the edge
    /// affinity, and `regard_w × (standing + dread × (1 − courage))` of each claimant.
    pub loyalty_w: f32,
    pub regard_w: f32,
    /// A loser with this many followers left leads a splinter (D36's `split`), else flees.
    pub splinter_min: usize,
}

impl Default for SuccessionCfg {
    fn default() -> Self {
        SuccessionCfg {
            enabled: true,
            war_margin: 0.02,
            war_min_members: 8,
            war_days: 14,
            war_grudge: 0.9,
            war_seconds: 2,
            war_grudge_second: 0.8,
            war_enemy_pairs: 30,
            loyalty_w: 0.3,
            regard_w: 0.3,
            splinter_min: 2,
        }
    }
}

/// M17 phase 2 (`[outside.parents]`, the body's `[outside]` and
/// `[outside.orders]` keys that phase 2 reads). Every value a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParentsCfg {
    /// The accounts at seed (the World is the Real economy's, not a row).
    pub factions: Vec<ParentRow>,
    pub market_sd: f32,
    pub market_revert: f32,
    /// A corp branch remits when its closing is at least this × `treasury_ref`.
    pub remit_floor: f32,
    /// The share of a branch's positive daily cashflow sent home.
    pub remit_share: f32,
    /// The `SetTariff` lever's value at seed.
    pub tariff: f32,
    /// A syndicate's gang pays this share of its treasury above `tribute_floor`, weekly.
    pub tribute_share: f32,
    pub tribute_floor: i64,
    pub rep_rate: f32,
    pub att_decay: f32,
    pub att_remit: f32,
    pub att_cost: f32,
    pub att_loss: f32,
    /// The city's Embargo cuts the parent's income by this (a sanction's cut).
    pub embargo_cut: f32,
    /// A gang's value a member (`branch_value`).
    pub member_value: i64,
    /// A gang's `cash`: treasury ÷ this.
    pub gang_cash_ref: i64,
    pub hysteresis: f32,
    pub min_hold_days: u32,
    /// The daily action's ceiling, a share of the parent's treasury.
    pub max_spend_frac: f32,
    pub subsidise_below: f32,
    pub subsidise_to: f32,
    /// Coins a day into one branch, at most.
    pub subsidy_cap_day: i64,
    pub flat_hold: f32,
    pub flat_subsidise: f32,
    // --- Phase 3: bodies.
    pub flat_reinforce: f32,
    pub flat_retake: f32,
    pub flat_found_branch: f32,
    pub flat_abandon: f32,
    /// A branch shock (a building lost, a raid) this recent opens Reinforce.
    pub recent_days: u32,
    pub reinforce_cooldown_days: u32,
    pub bodies_per_day: u32,
    pub bodies_per_call: u32,
    /// Living Outsiders per parent at most, at 2,000 residents (`scaled_to`).
    pub outsiders_cap: u32,
    /// Paid abroad per body (minted out).
    pub body_cost: i64,
    /// Into the branch per body (`Flow::ParentFund`): the new hire's wages.
    pub body_fund: i64,
    pub outsider_skill: f32,
    pub tour_days: u32,
    pub retake_window_days: u32,
    pub retake_wealth_min: f32,
    pub retake_premium: f32,
    /// A holder refusing this many buy-backs gets a `Retake` posted.
    pub refusals_to_force: u8,
    pub hit_lawfulness: f32,
    pub branch_seed: i64,
    pub upkeep_grace_days: u32,
    pub abandon_attitude: f32,
    pub abandon_days: u32,
    pub found_parent_treasury: i64,
    pub found_parent_share: f32,
    pub found_parent_days: u32,
    pub found_parent_frac: f32,
    pub parent_income_rate: f32,
    pub parent_upkeep_rate: f32,
    pub max_accounts: usize,
    // Syndicates.
    pub syndicate_subsidy: i64,
    pub brothers: u32,
    pub brother_delay_days: u32,
    pub brother_cost: i64,
    pub syndicate_cooldown_days: u32,
    pub refound_days: u32,
    pub refound_cost: i64,
    pub refound_attitude: f32,
    pub refound_window_days: u32,
    /// M17 phase 4b (Dylan, 2026-10-13: brothers as a rare event, not an
    /// occupancy): a syndicate sends brothers only to a gang sacked today
    /// with fewer than this many free members ...
    pub brothers_gutted_below: usize,
    /// ... on a keyed roll under this (the god `SendBrothers` aside).
    pub brothers_p: f32,
    // The State.
    /// The State's extra guards at most, at 2,000 residents (`scaled_to`).
    pub state_guards_max: u16,
    pub state_guards_step: u16,
    pub heat_murders: f32,
    pub mandate_heat: f32,
    pub mandate_bias: f32,
    pub mandate_days: u32,
    /// A State guard's day of wages in the weekly grant.
    pub grant_wage: i64,
    pub flat_fund: f32,
    pub flat_mandate: f32,
    pub flat_withdraw: f32,
    // --- Phase 4: the weapons that reach the ledger, and scrip.
    pub uplink_link_tier: u8,
    pub uplink_ice: u8,
    pub edge_per_unit: f32,
    pub edge_wipe: f32,
    pub edge_recover: f32,
    pub edge_tech: f32,
    pub edge_min: f32,
    pub edge_max: f32,
    pub data_floor: u32,
    pub data_per_day: [u32; 3],
    pub uplink_frac: f32,
    pub uplink_cap: i64,
    pub sanction_days: u32,
    pub sanction_cut: f32,
    pub sanction_cost: i64,
    pub sanction_regard: f32,
    pub state_sanction_heat: f32,
    pub campaign_margin: f32,
    pub flat_sanction: f32,
    pub flat_campaign: f32,
    pub death_days: u32,
    pub insolvent_days: u32,
    /// Data a parent ships a branch's Lab when its upkeep lapses, per tier (1-3).
    pub ship_data: [u32; 3],
    pub scrip_enabled: bool,
    pub scrip_wage_share: f32,
    pub scrip_par_min: f32,
    pub convert_min: i64,
    pub fixer_cut: f32,
    pub scrip_unrest: f32,
}

/// One seeded outside account. `treasury`, `income` and `upkeep` are per
/// branch (a brand dealt k times at 20k has k branches); the State's per
/// 2,000 residents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParentRow {
    pub name: String,
    pub kind: crate::outside::OutsideKind,
    /// A corp template (megacorps), a gang name (syndicates, by seeded
    /// order), or "Law" (the State).
    pub branch: String,
    pub treasury: i64,
    pub income: i64,
    pub upkeep: i64,
    pub exposure: f32,
    /// `[greed, pride, lawfulness]`.
    pub persona: [f32; 3],
    /// M17 phase 4: Data held abroad per track (Chrome, Deck, Industry).
    #[serde(default)]
    pub data: [u32; 3],
    /// M17 phase 4: the tech ceiling on its branches per track (0: none).
    #[serde(default)]
    pub tech: [u8; 3],
}

impl Default for ParentsCfg {
    fn default() -> Self {
        use crate::outside::OutsideKind::{Megacorp, State, Syndicate};
        let row = |name: &str, kind, branch: &str, t: i64, i: i64, u: i64, e: f32, p: [f32; 3]| ParentRow {
            name: name.to_string(),
            kind,
            branch: branch.to_string(),
            treasury: t,
            income: i,
            upkeep: u,
            exposure: e,
            persona: p,
            data: [0; 3],
            tech: [0; 3],
        };
        ParentsCfg {
            factions: vec![
                ParentRow {
                    data: [400, 1500, 400],
                    tech: [2, 3, 2],
                    ..row("Arasaka Global", Megacorp, "Arasaka", 100_000, 3000, 2500, 0.15, [0.6, 0.8, 0.3])
                },
                ParentRow {
                    data: [400, 1200, 600],
                    tech: [2, 3, 2],
                    ..row("Militech International", Megacorp, "Militech", 100_000, 2800, 2400, 0.2, [0.7, 0.6, 0.4])
                },
                row("The State", State, "Law", 60_000, 900, 900, 0.0, [0.2, 0.5, 0.8]),
                row("Hollow Brothers", Syndicate, "The Hollow", 3000, 40, 30, 0.3, [0.6, 0.7, 0.1]),
                row("Ninefold Lodge", Syndicate, "Ninefold", 3000, 40, 30, 0.3, [0.5, 0.8, 0.1]),
            ],
            market_sd: 0.01,
            market_revert: 0.1,
            remit_floor: 0.5,
            remit_share: 0.3,
            tariff: 0.0,
            tribute_share: 0.1,
            tribute_floor: 100,
            rep_rate: 0.05,
            att_decay: 0.05,
            att_remit: 0.3,
            att_cost: 0.4,
            att_loss: 0.3,
            embargo_cut: 0.08,
            member_value: 100,
            gang_cash_ref: 300,
            hysteresis: 0.10,
            min_hold_days: 3,
            max_spend_frac: 0.05,
            subsidise_below: 0.3,
            subsidise_to: 0.5,
            subsidy_cap_day: 1500,
            flat_hold: 0.3,
            flat_subsidise: 0.0,
            flat_reinforce: 0.0,
            flat_retake: 0.15,
            flat_found_branch: 0.0,
            flat_abandon: -0.2,
            recent_days: 14,
            reinforce_cooldown_days: 21,
            bodies_per_day: 2,
            bodies_per_call: 6,
            outsiders_cap: 24,
            body_cost: 300,
            body_fund: 300,
            outsider_skill: 0.6,
            tour_days: 60,
            retake_window_days: 30,
            retake_wealth_min: 0.2,
            retake_premium: 1.5,
            refusals_to_force: 2,
            hit_lawfulness: 0.4,
            branch_seed: 4000,
            upkeep_grace_days: 14,
            abandon_attitude: -0.5,
            abandon_days: 60,
            found_parent_treasury: 20000,
            found_parent_share: 0.5,
            found_parent_days: 30,
            found_parent_frac: 0.5,
            parent_income_rate: 0.02,
            parent_upkeep_rate: 0.017,
            max_accounts: 12,
            syndicate_subsidy: 150,
            brothers: 3,
            brother_delay_days: 5,
            brother_cost: 150,
            syndicate_cooldown_days: 30,
            refound_days: 21,
            refound_cost: 600,
            refound_attitude: -0.5,
            refound_window_days: 60,
            brothers_gutted_below: 6,
            brothers_p: 0.3,
            state_guards_max: 12,
            state_guards_step: 4,
            heat_murders: 10.0,
            mandate_heat: 0.6,
            mandate_bias: 0.2,
            mandate_days: 14,
            grant_wage: 8,
            flat_fund: 0.0,
            flat_mandate: 0.0,
            flat_withdraw: 0.0,
            uplink_link_tier: 3,
            uplink_ice: 3,
            edge_per_unit: 0.0003,
            edge_wipe: 0.15,
            edge_recover: 0.01,
            edge_tech: 0.02,
            edge_min: 0.2,
            edge_max: 1.5,
            data_floor: 100,
            data_per_day: [5, 10, 5],
            uplink_frac: 0.01,
            uplink_cap: 2000,
            sanction_days: 30,
            sanction_cut: 0.08,
            sanction_cost: 60,
            sanction_regard: -0.4,
            state_sanction_heat: 0.7,
            campaign_margin: 0.1,
            flat_sanction: -0.1,
            flat_campaign: -0.1,
            death_days: 21,
            insolvent_days: 7,
            ship_data: [0, 20, 40],
            scrip_enabled: true,
            scrip_wage_share: 0.3,
            scrip_par_min: 0.3,
            convert_min: 20,
            fixer_cut: 0.1,
            scrip_unrest: 0.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DemographyCfg {
    pub birth_p_per_day: f64,
    pub old_age_days: u32,
    pub old_age_p_per_day: f64,
    pub emigrate_mood: f32,
    pub emigrate_days: u32,
    // --- Jobs and room P6 (docs/JOBS_V2.md § 4, plan J19-J21): immigration
    // that answers the city. Read only with the market wage law.
    /// J19: the wage a migrant would earn outside (the Harris-Todaro `R`);
    /// the `SetOutsideWage` lever (`EconState.outside_wage_pin`) overrides it.
    #[serde(default = "default_outside_wage")]
    pub outside_wage: f32,
    /// J19: `pull = clamp((E − R) ÷ R, 0, pull_cap)`.
    #[serde(default = "default_pull_cap")]
    pub pull_cap: f32,
    /// J19: migrants a week at full pull, full beds and a factor of 1.
    #[serde(default = "default_migrants_max")]
    pub migrants_max: u32,
    /// J19: empty Block beds at which the beds term reads 1 (floored at 0.25).
    #[serde(default = "default_beds_ref")]
    pub beds_ref: f32,
    /// J20: a vacancy open this many days draws a recruited migrant.
    #[serde(default = "default_offer_days")]
    pub offer_days: u64,
    /// J20: recruited migrants a week at most (on top of the pull's week).
    #[serde(default = "default_offers_max")]
    pub offers_max: u32,
    /// J21: a migrant's age in years, `[lo, hi)`.
    #[serde(default = "default_migrant_age")]
    pub migrant_age: [f32; 2],
    /// J21: an offer's role skill is at least this.
    #[serde(default = "default_offer_skill")]
    pub offer_skill: f32,
    /// J21: the chance a migrant brings a spouse (into the same Block).
    #[serde(default = "default_p_spouse")]
    pub p_spouse: f64,
    /// Dylan's open question (spec, "Immigrant families"): children a
    /// couple may bring, at most (0: spouse only, the plan's J21).
    #[serde(default)]
    pub migrant_children: u8,
}

fn default_outside_wage() -> f32 {
    3.5
}
fn default_pull_cap() -> f32 {
    1.0
}
fn default_migrants_max() -> u32 {
    20
}
fn default_beds_ref() -> f32 {
    60.0
}
fn default_offer_days() -> u64 {
    3
}
fn default_offers_max() -> u32 {
    6
}
fn default_migrant_age() -> [f32; 2] {
    [18.0, 45.0]
}
fn default_offer_skill() -> f32 {
    0.4
}
fn default_p_spouse() -> f64 {
    0.2
}
