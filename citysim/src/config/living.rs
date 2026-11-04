use super::*;

/// Life pass L1 (docs/history/SHADOW_V1.md "Plan"): the cross-cutting causes that
/// made every shadowed resident's day a walk. `enabled = false` is the
/// master switch: every L1 branch reads it, so off is the ab79188 city.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LifeCfg {
    /// A discretionary goal's travel factor: `1 / (1 + walk / travel_half_ticks)`
    /// (a walk this long halves the score), floored at `travel_min`.
    pub travel_half_ticks: f32,
    pub travel_min: f32,
    /// A Sleep is not cut by the commute gate before this many ticks.
    pub sleep_min_ticks: u64,
    /// Below this energy Sleep takes `exhausted_flat` and may lie down
    /// where the agent stands when every bed is past `rough_min_tiles`.
    pub exhausted_energy: f32,
    pub exhausted_flat: f32,
    /// Sleep's flat at Night for anyone off shift (with energy below
    /// `ENERGY_SATISFIED`: the goal is skipped above it).
    pub night_sleep_flat: f32,
    pub rough_min_tiles: u32,
    /// S2 (`OPEN-289`, `OPEN-318`): a body starting a Beg, Scavenge or Sleep
    /// on a street tile where `spot_cap` others already stand steps to the
    /// nearest walkable tile within `spot_spill_tiles` with fewer
    /// (`fixes::spread_spot`; 0: off, a whole crowd on one tile).
    pub spot_cap: u16,
    pub spot_spill_tiles: u16,
    /// A housed agent's Sleep phase factor outside the Night and Evening
    /// (was 0.4 for everyone).
    pub housed_day_sleep: f32,
    /// A second bed (the Hideout, a Hotel) is taken when it is at least
    /// this many tiles nearer than Home.
    pub bed_margin_tiles: u32,
    /// Coins a housed agent keeps after a Hotel night (in meals).
    pub hotel_reserve_meals: i64,
    /// The homeless's Hotel reach (was `[street] hotel_reach`).
    pub hotel_reach_homeless: u32,
    /// The daily dole is paid where the agent is at 09:00 (no Hall walk).
    pub dole_in_place: bool,
    /// Days of dole paid at one Hall visit (accrued since the last one);
    /// 1 is the daily dole (off). Read only with `dole_in_place` off.
    pub dole_bulk_days: u64,
    /// The Hall is visited for the dole only with this many days accrued,
    /// unless the agent cannot buy a meal or the Hall is within `near_tiles`.
    pub dole_trip_days: u64,
    pub near_tiles: u32,
    /// An idle housed agent this far from Home idles where it is (or at
    /// its gang's nearer Hideout) instead of walking home to sit down.
    pub idle_far_tiles: u32,
    /// Arrest's "in shift" factor off shift (0: on shift only; was 0.5).
    pub arrest_off_shift: f32,
    /// A guard re-chases a suspect seen moving at most this many times per plan.
    pub chase_hops: u8,
    /// Pressure pass F2: a guard at the last-seen tile with the suspect out
    /// of sight chases on (a hop) to a sighting another guard made in the
    /// last this many ticks (the radio), not only to one in his own sight.
    pub chase_radio_ticks: u64,
    /// A suspect seen longer ago than this is chased only within `near_tiles`.
    pub arrest_fresh_ticks: u64,
    /// One `SawCrime` per (witness, actor, crime) within this many ticks.
    pub witness_dedupe_ticks: u64,
    /// New edges one arrival makes in a building (a Jail arrival: `jail_meet_max`).
    pub colocation_new_edges: usize,
    pub jail_meet_max: usize,
    /// In the cells an arrival also meets `jail_meet_max` gang members, and
    /// a member's pairs keep the cells' drift: the Jail is where gangs recruit.
    pub jail_pitch: bool,
    /// A gang order holds this long against a shock rescore unless the
    /// pending shocks reach `order_severe` (Retaliate and BreakOut exempt).
    pub order_dwell_ticks: u64,
    pub order_severe: f32,
    /// An employed adult commuting past this many tiles may move nearer.
    pub commute_cap_tiles: u32,
    pub relocate_per_day: usize,
    /// An employer does not rehire an agent who quit it for this many days.
    pub quit_rehire_days: u64,
    /// Execs: the youngest pick, the share of the corp's treasury paid as a
    /// day's salary (floored at `[economy] wage_exec`) and its cap, and the
    /// office shift at the corp's HQ.
    pub exec_min_age_years: f32,
    /// The exec is the greediest of the wealthiest tenth (else the wealthiest).
    pub exec_greed: bool,
    pub exec_pay_frac: f32,
    pub exec_pay_cap: i64,
    pub exec_shift: (u16, u16),
    /// Scavenge: coins the Recycler pays per haul (Treasury, `Flow::Sanitation`),
    /// found on `scavenge_p` of hours.
    pub scavenge_coins: i64,
    pub scavenge_p: f32,
    /// Shadow V4 bug 1: with no dole, a jobless adult keeps Earn (Beg,
    /// Scavenge) below this many meals' coins, not only below one.
    pub earn_meals_no_net: i64,
    /// Shadow V4 bug 2: Rest restores energy only up to here (Sleep is the
    /// full restore).
    pub rest_energy_cap: f32,
    /// Shadow V4 bug 3: Fight's "lawless" consideration, `1 − this ×
    /// lawfulness`, and its "odds" one, `this + the agent's strength share`.
    pub fight_lawful_damp: f32,
    pub fight_odds_floor: f32,
    /// An escort's walk to the Precinct is a van ride of at most this long.
    pub escort_van_ticks: u64,
    /// L2 (L29, `[lod] budget`): after this many dry Scavenge hours in a row
    /// `Earn` cools for `scavenge_cool_hours`.
    pub scavenge_dry_max: u8,
    pub scavenge_cool_hours: u64,
    /// ... and a shift cut short pays pro rata from this share of it.
    pub shift_pro_rata: f32,
    pub starving_hunger: f32,
    /// Shadow V4 bug 19: under this hunger, with a meal in reach, Eat breaks
    /// a shift (a meal break), and Work weighs 0.3 under it.
    pub shift_hunger_break: f32,
    pub sleep_wake_energy: f32,
    /// Workplace room (2026-10-11): a housed agent sleeping rough (out of
    /// doors, its bed past `rough_min_tiles`) sleeps on past
    /// `[needs] sleep_ticks_full` until `sleep_wake_energy` or the walk to
    /// the next shift, not woken at the cap to walk hours home at night
    /// (false: the cap ends it).
    pub rough_sleep_on: bool,
    pub idle_sleep_energy: f32,
    pub energy_brake: f32,
    /// Item 5: the free spot's walk cap (else the nearest spot), the lone
    /// hour's share of a HangOut's fun, the penalty per Enemy near a spot.
    pub spot_walk_cap_tiles: u32,
    pub spot_lone_fun: f32,
    pub spot_enemy_penalty: f32,
    /// Item 6: a fled-from tile is avoided by the spot pick this long, and
    /// Unwind and Socialise cool this long after the flight.
    pub avoid_spot_ticks: u64,
    pub flee_clear_ticks: u64,
    pub lod_dwell_ticks: u64,
    /// Item 10: guard-prisoner affinity in the cells stops at this.
    pub jail_affinity_cap: f32,
    /// Item 11: an employee who has missed this many workdays is dismissed
    /// (the absentee's dole at 2 was removed by roadmap addendum 17).
    pub noshow_fire_days: i64,
    /// Day gaps 3 (2026-10-11): a holder whose wealth is secure (`Needs::
    /// wealth` 1: Work's wealth term loses to Idle) who has missed this many
    /// workdays running quits the post (0: never).
    pub secure_quit_days: i64,
    /// Item 15: a Muster is held for the farthest member's walk (capped).
    pub muster_walk_cap_ticks: u64,
    /// Item 16: a Deal shift starts only from `deal_busy_from` to
    /// `deal_busy_to` (ticks of day, wrapping past midnight).
    pub deal_busy_from: u16,
    pub deal_busy_to: u16,
    /// Item 17: a teller does not tell the same listener the same deed
    /// again within this many days.
    pub told_cooldown_days: u64,
    /// Item 18: no freelance run where the target's ICE beats the attack by
    /// `[ice] flatline_gap` unless `U(wealth)` is at least this.
    pub hack_gap_wealth: f32,
    /// The violence fixes (2026-10-09, the wages-on diagnosis): three
    /// missing mechanisms that inflated off-screen killings and chrome
    /// episodes. Master switch (in flight); off is the 644eb10 city byte for
    /// byte. The sub-switches are read only with it on.
    pub violence_fixes: bool,
    /// Item 1: the ledger's `VictimClass::Street` (no Home): what evictees
    /// suffer on screen is learned and applied to the homeless only, not
    /// to every housed civilian of the district.
    pub vf_street_class: bool,
    /// ... its prior: the civilian class's (`[fviolence] class_mult`
    /// civilian) times this, per kind.
    pub vf_street_prior_mult: f32,
    /// Item 2: a riot's or an episode's rates are scaled by the hours it
    /// was live that day ÷ 24 (exposure is learned per 24 body-hours).
    pub vf_short_sources: bool,
    /// Item 3: no chrome purchase whose projected load takes the target
    /// sanity under `[chrome] psycho` (gang members, guards and Fighters
    /// excepted), and courage alone no longer unlocks the five-slot list.
    pub vf_chrome_cap: bool,
    /// Item 4: rent taken from a wage or the dole (`[rent] pay_from_income`)
    /// leaves an agent in arrears a meal's worth (`mean_price`) first.
    pub vf_rent_meal: bool,
    /// Item 5: an immigrant arrives with a seeded adult's savings for its
    /// Home's tier (`demography::spawn_immigrant`), crossed in from the World.
    pub vf_arrival: bool,
    /// Hunger as sickness P1 (docs/HUNGER_SICKNESS.md § 6): the Full
    /// planner's hunger gaps the recalibration diaries found. Master switch
    /// (off is the d771073 city byte for byte); the sub-switches are read
    /// only with it on.
    pub hunger_fixes: bool,
    /// G1: starving with a meal in reach, eat before any paid leisure, a
    /// bed, a dose or a purchase, and before an exhausted or night sleep.
    pub hf_food_first: bool,
    /// G2: Eat's can-obtain gate opens on a stocked Mission in reach.
    pub hf_mission: bool,
    /// G3: a theft of food costs `steal_can_buy` more while a meal is affordable.
    pub hf_buy_not_steal: bool,
    /// G4: a Beg with no giver is a dry hour (as Scavenge's), not a failure.
    pub hf_beg_retry: bool,
    /// P5f: a worker on shift does not leave for leisure (Unwind gated to 0
    /// on shift; it was `gate_or(0.3)`: 5.2 % of Full shift minutes in the
    /// all-Full flipped city went to Unwind).
    pub hf_shift_first: bool,
    /// P5g (Dylan: relocation): an employed worker whose commute
    /// (`life::commute_cost`, travel ticks) is past `commute_move_ticks`
    /// moves to the nearest Home with room within that of work (or trades
    /// Homes), needing only the first rent (no `rehouse_coins_mult`
    /// deposit), when it saves `commute_margin_ticks`; up to
    /// `relocate_per_1000` a day per 1,000 residents.
    pub hf_relocate: bool,
    pub commute_move_ticks: u64,
    /// Round 6 V4 (OPEN-240): a long commute: counted at the relocation's
    /// midnight read (`Jobs2Cols.commute_long`), and a retirement to kin
    /// does not move a working spouse into one (`careers::retire`).
    #[serde(default = "default_commute_long_ticks")]
    pub commute_long_ticks: u64,
    pub commute_margin_ticks: u64,
    pub relocate_per_1000: f32,
    /// P5g (Dylan: the hiring cap): a candidate within `commute_hire_ticks`
    /// (`life::commute_cost`) of the workplace is preferred over any beyond
    /// it (skill and the rehire bonus rank within each band); world creation
    /// houses the seeded staff near work where Homes allow (the relocation
    /// pass, uncapped).
    pub hf_hire_near: bool,
    pub commute_hire_ticks: u64,
    /// P5h (Dylan: the city grows toward jobs): an employer with at least
    /// `housing_demand_min` workers past `commute_move_ticks` and fewer free
    /// beds than that within `commute_move_ticks` of its door, for
    /// `housing_demand_days` midnights running, gets a Home on the nearest
    /// vacant Lot in that range (else a floor on the nearest Home there
    /// below `[floors] floors_max`); up to `housing_builds_per_1000` a day
    /// per 1,000 residents. Its owner corp pays `found_cost` / `floor_cost`
    /// when it can (company housing), else the Treasury buys it from the
    /// World when it can (keeping `[treasury] band[0]`), else nothing.
    pub hf_grow_housing: bool,
    /// P5i (coordinator call): meals around the shift. (a) At home before a
    /// shift its hunger would not last (under the Eat line by the shift's
    /// end), a worker eats from the pantry first (when a meal would not
    /// overfill it) and packs a meal to carry; (b) off shift, with the
    /// household's pantry and its carried meals under a shift's worth and
    /// coins for a meal, an idle worker makes a grocery run (buy, carry
    /// home, store) when the round trip ends `DEPARTURE_MARGIN` before the
    /// next shift's leaving time.
    pub hf_shift_meals: bool,
    pub grocery_score: f32,
    /// Item 11 (b), after the d19 diaries (up to 2.4 h a day of grocery
    /// walking, a third of runs ended by Sleep mid-walk): no grocery run
    /// starts that would leave the body under this energy by the time it is
    /// home (the energy decays over the round trip).
    pub grocery_energy_floor: f32,
    /// Item 11 (b): a run must bring home a meal for each this many ticks of
    /// its round trip (here to the shop and home): a 2 h trip for two meals,
    /// 7 h for seven. The d19 fighter walked 3.7 h each way for 1-2 meals; a
    /// poor man far from a shop shops rarely and big (0 turns it off).
    pub grocery_ticks_per_meal: u64,
    /// P5j (coordinator call): work-night sleep. The bedtime before a
    /// workday shift is the leaving time (`routine::must_leave_for_work`'s:
    /// the walk to work and `DEPARTURE_MARGIN` before the start) minus a
    /// full night (`[needs] sleep_ticks_full`) minus the walk home. Sleep
    /// gains `work_night_sleep_flat` x a pressure rising from 0 at
    /// `work_night_ramp_ticks` before bedtime to 1 at bedtime; past it,
    /// Unwind and the grocery run score x `work_night_unwind_mult` (a
    /// weight, not a ban: some stay out).
    pub hf_work_night: bool,
    /// Coordinator call (after the recalibration, 2026-10-10): with
    /// the market wage law, a homeless adult who holds a Job re-houses on the
    /// first rent (no `rehouse_coins_mult` deposit), as relocation does; the
    /// jobless keep the deposit. The flipped city had 82-92 employed adults
    /// on the street (~8 %), 930-1,420 beds empty, the deposit 21 coins on a
    /// ~6 a day wage.
    pub hf_rehouse_employed: bool,
    pub work_night_sleep_flat: f32,
    pub work_night_ramp_ticks: u64,
    pub work_night_unwind_mult: f32,
    pub housing_demand_min: u32,
    pub housing_demand_days: u16,
    pub housing_builds_per_1000: f32,
    /// The housing catch-up (`life::grow_housing`): with this > 0 a ready
    /// employer's unmet demand (far workers less the free beds in range)
    /// asks `ceil(unmet / a Home's beds / housing_catchup_days)` builds a
    /// day, the city's floored at `housing_builds_per_1000` and capped at
    /// `housing_builds_max_per_1000` per 1,000 residents, and a build keeps
    /// the employer's demand count. 0: the flat rule (one build per ready
    /// employer, the count reset by a build).
    pub housing_catchup_days: f32,
    pub housing_builds_max_per_1000: f32,
    /// The pre-roll (`life::preroll_housing`, world creation; with items
    /// 8-10, wages and no safety net on): Homes and floors toward the
    /// seeded staff's jobs, unpaid as the seed buildings, until the far
    /// workers have beds in range or no Lot or floor is left there.
    pub housing_preroll: bool,
    /// G2: a starving agent walks this much further to a Mission.
    pub hungry_reach_mult: f32,
    /// G3: the theft cost added while the agent can buy a meal.
    pub steal_can_buy: f32,
    /// G4: dry Begs in a row that cool Earn ...
    pub beg_dry_max: u8,
    /// ... for this many hours.
    pub beg_cool_hours: u64,
    /// Lives at work phase 7 (§ 5.3), under the `[lives]` master
    /// (`[life] exec_home_tier = 0`; 0 is off and not written): a corp
    /// exec's Home is of this tier or better (2: the Spire) when one has
    /// room, at seed, daily, and on relocation; no one is evicted for it.
    #[serde(skip_serializing_if = "is_zero_u8_cfg")]
    pub exec_home_tier: u8,
    /// § 5.3: the Spire's Homes are checked at seed against `execs ×
    /// exec_home_spare` beds and the result printed (never an eviction).
    #[serde(skip_serializing_if = "is_zero_f32_cfg")]
    pub exec_home_spare: f32,
    /// The perf pass phase 10 (ISLAND_PASS I9, a label is not space): an
    /// exec's Home stands within this many tiles (Manhattan, door to door)
    /// of his corp's Hq (`life::hq_of`). `life::house_exec` moves an exec
    /// housed under `exec_home_tier` into the free Home of that tier nearest
    /// the Hq (not the emptiest), and one housed at the tier but further
    /// than this into a nearer one within it when one has room. 0 is off (the
    /// base pick) and not written.
    #[serde(default, skip_serializing_if = "is_zero_u16_cfg")]
    pub exec_home_reach: u16,
}

impl Default for LifeCfg {
    fn default() -> Self {
        LifeCfg {
            travel_half_ticks: 120.0,
            travel_min: 0.15,
            sleep_min_ticks: 240,
            exhausted_energy: 0.12,
            exhausted_flat: 0.6,
            night_sleep_flat: 0.2,
            rough_min_tiles: 45,
            spot_cap: 3,
            spot_spill_tiles: 3,
            housed_day_sleep: 0.25,
            bed_margin_tiles: 20,
            hotel_reserve_meals: 2,
            hotel_reach_homeless: 120,
            dole_in_place: true,
            dole_bulk_days: 1,
            dole_trip_days: 1,
            near_tiles: 30,
            idle_far_tiles: 30,
            arrest_off_shift: 0.0,
            chase_hops: 3,
            chase_radio_ticks: 30,
            arrest_fresh_ticks: 120,
            witness_dedupe_ticks: 360,
            colocation_new_edges: 2,
            jail_meet_max: 2,
            jail_pitch: true,
            order_dwell_ticks: 1440,
            order_severe: 1.0,
            commute_cap_tiles: 30,
            relocate_per_day: 8,
            quit_rehire_days: 7,
            exec_min_age_years: 30.0,
            exec_greed: true,
            exec_pay_frac: 0.002,
            exec_pay_cap: 40,
            exec_shift: (540, 1020),
            scavenge_coins: 1,
            scavenge_p: 0.15,
            earn_meals_no_net: 3,
            rest_energy_cap: 0.6,
            fight_lawful_damp: 0.6,
            fight_odds_floor: 0.5,
            escort_van_ticks: 60,
            scavenge_dry_max: 3,
            scavenge_cool_hours: 4,
            shift_pro_rata: 0.5,
            starving_hunger: 0.1,
            shift_hunger_break: 0.12,
            sleep_wake_energy: 0.99,
            rough_sleep_on: false,
            idle_sleep_energy: 0.9,
            energy_brake: 0.5,
            spot_walk_cap_tiles: 22,
            spot_lone_fun: 0.4,
            spot_enemy_penalty: 0.5,
            avoid_spot_ticks: 1440,
            flee_clear_ticks: 30,
            lod_dwell_ticks: 180,
            jail_affinity_cap: 0.3,
            noshow_fire_days: 7,
            secure_quit_days: 2,
            muster_walk_cap_ticks: 480,
            deal_busy_from: 720,
            deal_busy_to: 120,
            told_cooldown_days: 3,
            hack_gap_wealth: 0.9,
            violence_fixes: false,
            vf_street_class: true,
            vf_street_prior_mult: 3.0,
            vf_short_sources: true,
            vf_chrome_cap: true,
            vf_rent_meal: true,
            vf_arrival: true,
            hunger_fixes: false,
            hf_food_first: true,
            hf_mission: true,
            hf_buy_not_steal: true,
            hf_beg_retry: true,
            hf_shift_first: true,
            hf_relocate: true,
            commute_move_ticks: 80,
            commute_long_ticks: 180,
            commute_margin_ticks: 40,
            relocate_per_1000: 20.0,
            hf_hire_near: true,
            commute_hire_ticks: 80,
            hf_grow_housing: true,
            hf_shift_meals: true,
            grocery_score: 0.5,
            grocery_energy_floor: 0.4,
            grocery_ticks_per_meal: 60,
            hf_work_night: true,
            hf_rehouse_employed: true,
            work_night_sleep_flat: 0.5,
            work_night_ramp_ticks: 120,
            work_night_unwind_mult: 0.3,
            housing_demand_min: 6,
            housing_demand_days: 3,
            housing_builds_per_1000: 1.0,
            housing_catchup_days: 5.0,
            housing_builds_max_per_1000: 6.0,
            housing_preroll: true,
            hungry_reach_mult: 2.0,
            steal_can_buy: 12.0,
            beg_dry_max: 3,
            beg_cool_hours: 1,
            exec_home_tier: 0,
            exec_home_spare: 0.0,
            exec_home_reach: 0,
        }
    }
}

/// L2 (plan L5): the master switch of the living city.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LivingCfg {
    /// L2 (coordinator ruling on M14, 2026-10-08): a corp keeps its standing
    /// order this many days after taking it against a shock rescore ...
    pub corp_order_dwell_days: u64,
    /// ... unless the challenger beats it by this margin, or the pending
    /// shocks reach `[life] order_severe` (the gangs' dwell rule, L1).
    pub corp_order_margin: f32,
    /// L2 phase 5 (gang desistance, `gang::desist`): an ordinary member's
    /// daily chance to walk away, times the terms below (0: no desistance).
    pub desist_base: f32,
    /// x this while the member holds a Job that has paid a wage.
    pub desist_employed: f32,
    /// x `desist_unpaid` when no stipend or tribute reached it for this many days.
    pub desist_unpaid_days: u64,
    pub desist_unpaid: f32,
    /// x this when married or housed outside the Sump districts.
    pub desist_settled: f32,
    /// x `desist_aged` from this age (sim years).
    pub desist_age: u32,
    pub desist_aged: f32,
    /// x this when released from a sentence in the last 30 days.
    pub desist_jailed: f32,
    /// x this (a brake) with an unsettled grudge on a rival gang's member or
    /// the gang in an open vendetta.
    pub desist_feud: f32,
    /// x this (a brake) for a member the gang has invested in or earns
    /// through: the keeper of an asset its gang owns (Arms, a deck), or a
    /// deal in the last 7 days.
    pub desist_invested: f32,
    /// A leaver's `JoinGang` cooldown, days.
    pub desist_cooldown_days: u64,
    /// L2 phase 5 (`economy::reserve_release`): a Market priced above this
    /// and under `restock_floor` gets Reserve food free (0: off) ...
    pub reserve_release_price: i64,
    /// ... up to this many units a day.
    pub reserve_release_batch: u32,
}

impl Default for LivingCfg {
    fn default() -> Self {
        LivingCfg {
            corp_order_dwell_days: 3,
            corp_order_margin: 0.05,
            desist_base: 0.0,
            desist_employed: 4.0,
            desist_unpaid_days: 14,
            desist_unpaid: 3.0,
            desist_settled: 2.0,
            desist_age: 35,
            desist_aged: 2.0,
            desist_jailed: 2.0,
            desist_feud: 0.25,
            desist_invested: 0.25,
            desist_cooldown_days: 60,
            reserve_release_price: 0,
            reserve_release_batch: 200,
        }
    }
}

/// L2 § 1: the venues and Fabs seeded at day 0 (`jobs::seed_venues`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeedVenuesCfg {
    pub club: u32,
    pub arcade: u32,
    pub noodle_bar: u32,
    pub fight_pit: u32,
    pub den: u32,
    pub lounge: u32,
    pub fab: u32,
}

impl Default for SeedVenuesCfg {
    fn default() -> Self {
        SeedVenuesCfg { club: 3, arcade: 3, noodle_bar: 10, fight_pit: 2, den: 2, lounge: 1, fab: 2 }
    }
}

impl SeedVenuesCfg {
    pub fn count(&self, kind: BuildingKind) -> u32 {
        match kind {
            BuildingKind::Club => self.club,
            BuildingKind::Arcade => self.arcade,
            BuildingKind::NoodleBar => self.noodle_bar,
            BuildingKind::FightPit => self.fight_pit,
            BuildingKind::Den => self.den,
            BuildingKind::Lounge => self.lounge,
            BuildingKind::Fab => self.fab,
            _ => 0,
        }
    }
}

/// L2 § 1 `[jobs]` (plan L5: `LivingJobsCfg`; `JobsCfg` is `[world.jobs]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LivingJobsCfg {
    pub seed_venues: SeedVenuesCfg,
    /// Of `found_cost`: a refit of a derelict Block.
    pub refit_frac: f32,
    pub fab_yield: f32,
    pub fab_skill_floor: f32,
    pub fab_skill_slope: f32,
    /// Coins imported in 14 days by a Tech corp's own sellers before its
    /// `Grow` builds a Fab.
    pub fab_import_trigger: i64,
    pub scrap_per_part: u32,
    /// Litter units a sweeper clears per hour of `Sweep`.
    pub sweep_per_hour: u32,
    /// Plan L3: a Market's full staff with jobs on (`[buildings]
    /// market.staff` stays the off value).
    pub market_staff: u32,
    /// Plan L3: a Bar's.
    pub bar_staff: u32,
    /// Plan L8: Sanitation sweeps its beat (false: TendGraves and the D23 ledger).
    pub sweep: bool,
}

impl Default for LivingJobsCfg {
    fn default() -> Self {
        LivingJobsCfg {
            seed_venues: SeedVenuesCfg::default(),
            refit_frac: 0.5,
            fab_yield: 0.6,
            fab_skill_floor: 0.6,
            fab_skill_slope: 0.8,
            fab_import_trigger: 300,
            scrap_per_part: 4,
            sweep_per_hour: 6,
            market_staff: 12,
            bar_staff: 5,
            sweep: true,
        }
    }
}

/// L2 § 1: a venue's price by tier (Sump, Mid, Spire); 0 = not sold there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PriceBaseCfg {
    pub club: [i64; 3],
    pub arcade: [i64; 3],
    /// A NoodleBar meal: the Market's price plus this.
    pub noodle_bar_markup: i64,
    pub fight_pit: [i64; 3],
    pub den: [i64; 3],
    pub lounge: [i64; 3],
}

impl Default for PriceBaseCfg {
    fn default() -> Self {
        PriceBaseCfg {
            club: [0, 5, 12],
            arcade: [2, 3, 6],
            noodle_bar_markup: 1,
            fight_pit: [3, 0, 0],
            den: [2, 3, 0],
            lounge: [0, 0, 25],
        }
    }
}

impl PriceBaseCfg {
    /// The tier price of a leisure kind (a NoodleBar's is its markup).
    pub fn of(&self, kind: BuildingKind, tier: u8) -> i64 {
        let t = usize::from(tier.min(2));
        match kind {
            BuildingKind::Club => self.club[t],
            BuildingKind::Arcade => self.arcade[t],
            BuildingKind::NoodleBar => self.noodle_bar_markup,
            BuildingKind::FightPit => self.fight_pit[t],
            BuildingKind::Den => self.den[t],
            BuildingKind::Lounge => self.lounge[t],
            _ => 0,
        }
    }
}

/// L2 § 2: the fun a satisfier gives (plan keys; the spec's leisure-kinds
/// table and rung ladder).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FunGainCfg {
    pub club: f32,
    /// A Club priced at the Spire tier (the high rung).
    pub club_spire: f32,
    pub arcade: f32,
    pub noodle_bar: f32,
    pub fight_pit: f32,
    pub den: f32,
    pub lounge: f32,
    /// A drink (a Bar, a Club, a Den).
    pub drink: f32,
    /// An hour of HangOut.
    pub hangout_hour: f32,
    /// Watching a raid or a bout from the door (an hour).
    pub watch: f32,
    /// The free rung off screen (`stat_daily`).
    pub free_stat: f32,
}

impl Default for FunGainCfg {
    fn default() -> Self {
        FunGainCfg {
            club: 0.6,
            club_spire: 0.7,
            arcade: 0.35,
            noodle_bar: 0.1,
            fight_pit: 0.5,
            den: 0.5,
            lounge: 0.9,
            drink: 0.1,
            hangout_hour: 0.1,
            watch: 0.3,
            free_stat: 0.3,
        }
    }
}

/// L2 § 1-2 `[leisure]`. Phase 1 reads the price table and the front
/// markup; phase 2 owns `enabled` and the rest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LeisureCfg {
    pub price_base: PriceBaseCfg,
    pub front_markup: f32,
    // --- phase 2 (spec § 1-2 and plan keys).
    pub house_edge: f32,
    pub stake_frac: f32,
    pub stake_cap: i64,
    /// A win above this posts `Gambled`.
    pub big_win: i64,
    pub bout_hour: u16,
    /// Round 6 V6 (OPEN-217): a pit's card is one bout a night for every
    /// this many Fighters on shift in it (at least one): the bout went to
    /// the two longest since their last, one a night whatever the roster.
    pub bout_card_per: u16,
    /// Round 7 W6 (OPEN-217): the bout's winner takes a purse from the
    /// house (round 8 X5: `Flow::Purse`, the pit's owner, off the table's
    /// take): this many coins ...
    pub bout_purse: i64,
    /// ... plus this share of the main event's stakes (0 and 0: no purse).
    pub bout_purse_share: f32,
    /// Round 8 X5 (OPEN-217): the bout's margin (how far inside the
    /// winner's chance the night's draw fell) at or over which the loser is
    /// beaten (hurt, a wound); under it he loses on points.
    pub bout_beating: f32,
    /// Round 8 X5: the energy a loser on points loses (no wound).
    pub bout_points_tired: f32,
    /// The beaten loser is hurt (a wound to have seen to,
    /// `customers::hurt`): his energy and safety fall by this ...
    pub bout_hurt: f32,
    /// ... plus this x the bout's margin past `bout_beating` (0 and 0: the
    /// base 0.3 on every loss and no wound) ...
    pub bout_hurt_margin: f32,
    /// ... and at this fall or more the line reads "badly hurt".
    pub bout_hurt_bad: f32,
    /// A Club's door Host refuses a visitor with `rep.heat` at least this
    /// when the Host's courage beats the heat.
    pub door_heat: f32,
    /// Corp, Street, Dreg.
    pub class_mult: [f32; 3],
    pub hangout_ticks: u64,
    /// Socialise's "no drink possible" clause is dropped with a spot this near.
    pub hangout_reach: u32,
    pub barrels_per_sump: u32,
    pub kin_w: f32,
    pub watch_tiles: u32,
    pub fronts_max: u32,
    pub crackdown_close_days: u64,
    /// 0 = day 0 of the week (`day % 7`).
    pub collect_weekday: u64,
    pub tribute_share: f32,
    pub call_tiles: u32,
    pub call_hours: u64,
    pub preach_share: f32,
    pub preach_opinion: f32,
    /// Plan key (L20): the Hideout spot's score bonus during a Call.
    pub call_w: f32,
    /// Plan key: courage above which the mid rung's FightPit and Den
    /// outscore the Club (`(0.5 + courage)` below it is halved).
    pub gamble_rung_courage: f32,
    /// Plan key: the fun each satisfier gives.
    pub gain: FunGainCfg,
    /// Plan key (deviation): the evening shift of the Hosts, Fighters,
    /// Croupiers and Concierges (so a bout at `bout_hour` has Fighters on
    /// shift); Attendants and Cooks keep `[world] shift_day`.
    pub evening_shift: Vec<(u16, u16)>,
    /// Plan key: hours before `bout_hour` a FightPit takes bets.
    pub bet_window_hours: u16,
    /// Day gaps 1 (2026-10-11): a job holder's off hours end this many
    /// ticks before leaving for the next workday shift (`leisure::off_hours`:
    /// the agent's own shift, not the wall clock's Evening and Night).
    pub shift_lead_ticks: u64,
    /// Day gaps 2 (2026-10-11): a paid venue farther than this walk
    /// (`exec::timed_walk`, the commute's estimate: a vehicle reaches
    /// farther) is not on a Full agent's Unwind ladder (fabbers walked 171
    /// min to a Den after a 9 h shift).
    pub venue_walk_cap_ticks: u64,
}

impl Default for LeisureCfg {
    fn default() -> Self {
        LeisureCfg {
            price_base: PriceBaseCfg::default(),
            front_markup: 1.2,
            house_edge: 0.08,
            stake_frac: 0.3,
            stake_cap: 20,
            big_win: 40,
            bout_hour: 23,
            bout_card_per: 4,
            bout_purse: 0,
            bout_purse_share: 0.0,
            bout_beating: 0.0,
            bout_points_tired: 0.0,
            bout_hurt: 0.0,
            bout_hurt_margin: 0.0,
            bout_hurt_bad: 1.0,
            door_heat: 0.6,
            class_mult: [1.3, 1.0, 0.8],
            hangout_ticks: 60,
            hangout_reach: 40,
            barrels_per_sump: 3,
            kin_w: 0.1,
            watch_tiles: 12,
            fronts_max: 2,
            crackdown_close_days: 3,
            collect_weekday: 6,
            tribute_share: 0.5,
            call_tiles: 120,
            call_hours: 3,
            preach_share: 0.5,
            preach_opinion: 0.05,
            call_w: 2.0,
            gamble_rung_courage: 0.5,
            gain: FunGainCfg::default(),
            evening_shift: vec![(1080, 1440)],
            bet_window_hours: 2,
            shift_lead_ticks: 120,
            venue_walk_cap_ticks: 60,
        }
    }
}

/// L2 § 1 `[budget]`: the Treasury's band and public works.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BudgetCfg {
    /// `[lo, hi]` Treasury coins.
    pub band: [i64; 2],
    /// Plan key: days in a row on one side before the band acts (spec 3).
    pub band_hold_days: u16,
    pub works_step: u16,
    pub works_max: u16,
    pub upkeep_step: f32,
    pub upkeep_floor: f32,
    /// L2 phase 5 (plan key): a public-works hire's daily wage; 0 keeps
    /// `[economy] wage_sanitation` (the M12 sweeper's wage).
    pub works_wage: i64,
}

impl Default for BudgetCfg {
    fn default() -> Self {
        BudgetCfg {
            band: [30_000, 60_000],
            band_hold_days: 3,
            works_step: 10,
            works_max: 120,
            upkeep_step: 0.05,
            upkeep_floor: 0.3,
            works_wage: 0,
        }
    }
}

fn default_commute_long_ticks() -> u64 {
    180
}
