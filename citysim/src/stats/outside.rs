//! M17: the parents' group.

use serde::{Deserialize, Serialize};

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

cols! {
    /// M17 phase 2: the parents' group. The flows are the last midnight's
    /// (the parents act at midnight); the crossings are the whole day's, every
    /// account (the World's too).
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct OutsideCols {
        /// Coins the branches sent home (net of the tariff), the tariff the
        /// Treasury kept, the subsidies paid and their count.
        pub remit: i64 => col("outside_remit"),
        pub tariff: i64 => col("outside_tariff"),
        pub parent_fund: i64 => col("outside_parent_fund"),
        pub subsidies: u32 => col("outside_subsidies"),
        /// Coins crossing into and out of the city today, every account.
        pub crossed_in: i64 => col("outside_in"),
        pub crossed_out: i64 => with(
            |h| h.extend(["outside_out", "outside_net"].map(String::from)),
            |s, v, _| v.extend([s.crossed_out.to_string(), (s.crossed_in - s.crossed_out).to_string()])
        ),
        /// Parents failing (income under upkeep for a week).
        pub failing: u32 => col("parents_failing"),
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
        pub promoted: u32 => col("promoted_cast"),
        pub cast_arrived: u32,
        pub breaches: u32 => col("blackwall_breaches"),
        pub accounts: Vec<ParentCols> => with(
            |_| {},
            |s, v, _| {
                for a in &s.accounts {
                    v.push(a.treasury.to_string());
                    v.push(a.income.to_string());
                    v.push(format!("{:.3}", a.health));
                    v.push(format!("{:.3}", a.attitude));
                    v.push(format!("{:.3}", a.rep));
                    v.push(format!("{:.3}", a.branch_loss));
                    v.push(a.order.clone());
                    v.push(format!("{:.3}", a.edge));
                }
            }
        ),
    }
}

impl OutsideCols {
    /// The header's names for `n` parents: [`OutsideCols::header`] and each
    /// parent's `o{i}_*` slot (`accounts`, one per parent, prints them).
    pub fn header_for(n: usize) -> String {
        let mut h = Vec::new();
        Self::header_into(&mut h);
        for i in 1..=n {
            for c in ["treasury", "income", "health", "attitude", "rep", "loss", "order", "edge"] {
                h.push(format!("o{i}_{c}"));
            }
        }
        h.join(",")
    }
}
