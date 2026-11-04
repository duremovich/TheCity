//! The Real economy's group.

use super::DISTRICT_SLOTS;
use crate::components::MAX_DISTRICTS;
use serde::{Deserialize, Serialize};

cols! {
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
        pub appetite_food: f32 => fmt("{:.3}"),
        pub appetite_parts: f32 => fmt("{:.3}"),
        pub appetite_data: f32 => fmt("{:.3}"),
        /// Today's first-unit Food bid.
        pub bid_food: f32 => fmt("{:.3}"),
        pub ask_food: i64,
        pub food_imported: u32,
        pub food_exported: u32,
        pub wage_mult_mean: f32 => fmt("{:.3}"),
        pub wage_gross_mean: f32 => fmt("{:.3}"),
        pub vacancies_open: u32,
        pub laid_off: u32,
        pub pop_inflow_wages: i64,
        pub pop_inflow_other: i64,
        pub tax_rate: f32 => fmt("{:.3}"),
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
        pub d_meals: [u32; MAX_DISTRICTS] => with(
            |h| {
                for i in 1..=DISTRICT_SLOTS {
                    h.extend(["meals", "starving", "dregs"].map(|c| format!("d{i}_{c}")));
                }
            },
            |s, v, d| {
                for i in 0..d.min(MAX_DISTRICTS) {
                    v.extend([s.d_meals[i].to_string(), s.d_starving[i].to_string(), s.d_dregs[i].to_string()]);
                }
            }
        ),
        #[serde(deserialize_with = "crate::components::district_slots_any_len")]
        pub d_starving: [u32; MAX_DISTRICTS] => skip,
        #[serde(deserialize_with = "crate::components::district_slots_any_len")]
        pub d_dregs: [u32; MAX_DISTRICTS] => skip,
    }
}

impl EconCols {
    pub fn is_zero(&self) -> bool {
        *self == EconCols::default()
    }
}
