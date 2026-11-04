//! Jobs v2: the jobs pass's group.

use serde::{Deserialize, Serialize};

cols! {
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
}

impl JobsCols {
    pub fn is_zero(&self) -> bool {
        *self == JobsCols::default()
    }
}
