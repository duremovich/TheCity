//! The Statistical tier's policy (experiment, `docs/EXPERIMENT_LEARNED_STAT_POLICY.md`):
//! what turns an agent's state at the start of its hour into the hour's
//! probabilities (a `StatRow`). Two impls behind `[lod] policy`:
//!
//! - `"table"` (default): the calibrated 24-row table (`assets/stat_table.toml`),
//!   keyed on phase, lawfulness bucket and hunger bucket.
//! - `"mlp"`: two small 2-layer networks (`assets/stat_mlp.toml`, trained
//!   offline on `calibrate --rows-csv` output) over the extended feature
//!   vector `features`. The table still supplies the two city-wide numbers
//!   (`p_dole_day`, `p_theft_caught`).
//!
//! The policy only changes the probabilities: `run_statistical` draws the
//! same rolls in the same order from the agent's stream whichever is used.

use serde::{Deserialize, Serialize};

use crate::components::{Identity, Mood, Personality, Position};
use crate::entity::EntityId;
use crate::world::{StatRow, StatTable, World};

/// Length of `features`.
pub const N_FEATURES: usize = 17;

/// Column names of `features`, in order (the `calibrate --rows-csv` header).
pub const FEATURE_NAMES: [&str; N_FEATURES] = [
    "ph_morning",
    "ph_work",
    "ph_evening",
    "ph_night",
    "lawfulness",
    "hunger",
    "wealth",
    "employed",
    "housed",
    "married",
    "age",
    "mood",
    "z_spire",
    "z_civic",
    "z_vats",
    "z_mid",
    "z_sump",
];

/// Wealth is coins over the local Market price, clamped to this many meals
/// and scaled to `0..=1`.
const WEALTH_MEALS: f32 = 20.0;

/// The agent's state at the start of its hour: phase one-hot (Morning, Work,
/// Evening, Night), lawfulness, hunger, wealth (meals of coins / 20,
/// clamped), employed, housed, married, age (years / 100), mood (`-1..=1`),
/// zone one-hot (`Zone::ALL` order). Season is left out: one calibration city
/// runs 30 days, one season.
pub fn features(world: &World, id: EntityId) -> [f32; N_FEATURES] {
    let mut f = [0.0f32; N_FEATURES];
    f[StatTable::phase_index(world.phase())] = 1.0;
    f[4] = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
    f[5] = world.comp::<crate::components::Needs>(id).map_or(1.0, |n| n.hunger);
    let coins = world.comp::<crate::components::Wallet>(id).map_or(0, |w| w.coins);
    let price = world.local(id, crate::components::BuildingKind::Market).map_or(1, |m| world.price_for(m, id)).max(1);
    f[6] = (coins as f32 / price as f32).clamp(0.0, WEALTH_MEALS) / WEALTH_MEALS;
    f[7] = f32::from(u8::from(world.has::<crate::components::Job>(id)));
    f[8] = f32::from(u8::from(world.comp::<crate::components::Household>(id).is_some_and(|h| h.home.is_some())));
    f[9] = f32::from(u8::from(world.spouse_of(id).is_some()));
    f[10] = world.comp::<Identity>(id).map_or(0.3, |i| i.age_days as f32 / (100 * crate::time::DAYS_PER_YEAR) as f32);
    f[11] = world.comp::<Mood>(id).map_or(0.0, |m| m.value);
    let zone = world.comp::<Position>(id).map_or(crate::components::Zone::Mid, |p| world.map.zone(p.tile));
    f[12 + zone.index()] = 1.0;
    f
}

/// What the Statistical hour reads: the agent's probabilities for this hour.
pub trait StatPolicy {
    fn row(&self, world: &World, id: EntityId) -> StatRow;
}

/// The calibrated table (the default).
pub struct TablePolicy<'a>(pub &'a StatTable);

impl StatPolicy for TablePolicy<'_> {
    fn row(&self, world: &World, id: EntityId) -> StatRow {
        let lawfulness = world.comp::<Personality>(id).map_or(0.5, |p| p.lawfulness);
        let hunger = world.comp::<crate::components::Needs>(id).map_or(1.0, |n| n.hunger);
        self.0.row(world.phase(), lawfulness, hunger).numbers()
    }
}

/// One dense 2-layer net: `inputs` (indices into `features`) -> ReLU hidden ->
/// raw outputs. The feature standardisation is folded into `w1` and `b1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Net {
    pub inputs: Vec<usize>,
    pub hidden: usize,
    pub outputs: usize,
    /// `hidden x inputs`, row-major.
    pub w1: Vec<f32>,
    pub b1: Vec<f32>,
    /// `outputs x hidden`, row-major.
    pub w2: Vec<f32>,
    pub b2: Vec<f32>,
}

/// Widest hidden layer and output vector `Net::forward` supports.
const MAX_HIDDEN: usize = 64;
const MAX_OUT: usize = 8;

impl Net {
    fn check(&self, what: &str) {
        let n = self.inputs.len();
        assert!(self.hidden <= MAX_HIDDEN && self.outputs <= MAX_OUT, "stat_mlp {what}: too wide");
        assert!(self.inputs.iter().all(|&i| i < N_FEATURES), "stat_mlp {what}: input index");
        assert_eq!(self.w1.len(), self.hidden * n, "stat_mlp {what}: w1");
        assert_eq!(self.b1.len(), self.hidden, "stat_mlp {what}: b1");
        assert_eq!(self.w2.len(), self.outputs * self.hidden, "stat_mlp {what}: w2");
        assert_eq!(self.b2.len(), self.outputs, "stat_mlp {what}: b2");
    }

    /// Raw outputs (logits) into `out[..outputs]`.
    pub fn forward(&self, x: &[f32; N_FEATURES], out: &mut [f32; MAX_OUT]) {
        let n = self.inputs.len();
        let mut h = [0.0f32; MAX_HIDDEN];
        for (j, hj) in h.iter_mut().enumerate().take(self.hidden) {
            let w = &self.w1[j * n..(j + 1) * n];
            let s = self.inputs.iter().zip(w).fold(self.b1[j], |s, (&i, &wi)| s + wi * x[i]);
            *hj = s.max(0.0);
        }
        for (k, o) in out.iter_mut().enumerate().take(self.outputs) {
            let w = &self.w2[k * self.hidden..(k + 1) * self.hidden];
            *o = w.iter().zip(&h[..self.hidden]).fold(self.b2[k], |s, (wi, hi)| s + wi * hi);
        }
    }
}

/// `assets/stat_mlp.toml`: the outcome net (5 logits: eat, work, social,
/// sleep, idle; softmax) and the roll net (8 logits: steal, flirt, robbed,
/// assaulted, killed, meet, chat, chat_home; sigmoid each). The roll net
/// leaves hunger out, as the table pools its rolls over hunger: a
/// Statistical agent eats as soon as it is hungry, so hunger-conditioned
/// crime rates would not transfer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatMlp {
    pub version: u32,
    pub outcome: Net,
    pub rolls: Net,
}

impl StatMlp {
    pub fn load(config: &crate::Config) -> StatMlp {
        let path = config.asset("stat_mlp.toml");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let m: StatMlp = toml::from_str(&text).unwrap_or_else(|e| panic!("bad {}: {e}", path.display()));
        assert_eq!(m.outcome.outputs, 5, "stat_mlp outcome: 5 outputs");
        assert_eq!(m.rolls.outputs, 8, "stat_mlp rolls: 8 outputs");
        m.outcome.check("outcome");
        m.rolls.check("rolls");
        m
    }

    /// The hour's row for a feature vector.
    pub fn row_for(&self, x: &[f32; N_FEATURES]) -> StatRow {
        let mut o = [0.0f32; MAX_OUT];
        self.outcome.forward(x, &mut o);
        let m = o[..5].iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut e = [0.0f32; 5];
        for (ek, &ok) in e.iter_mut().zip(&o[..5]) {
            *ek = (ok - m).exp();
        }
        let z: f32 = e.iter().sum();
        let mut r = [0.0f32; MAX_OUT];
        self.rolls.forward(x, &mut r);
        let sig = |v: f32| 1.0 / (1.0 + (-v).exp());
        StatRow {
            label: String::new(),
            p_eat: e[0] / z,
            p_work: e[1] / z,
            p_social: e[2] / z,
            p_sleep: e[3] / z,
            p_steal: sig(r[0]),
            p_flirt: sig(r[1]),
            p_robbed: sig(r[2]),
            p_assaulted: sig(r[3]),
            p_killed: sig(r[4]),
            p_meet: sig(r[5]),
            p_chat: sig(r[6]),
            p_chat_home: sig(r[7]),
        }
    }
}

/// The learned policy.
pub struct MlpPolicy<'a>(pub &'a StatMlp);

impl StatPolicy for MlpPolicy<'_> {
    fn row(&self, world: &World, id: EntityId) -> StatRow {
        self.0.row_for(&features(world, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(inputs: Vec<usize>, w1: Vec<f32>, b1: Vec<f32>, outputs: usize, w2: Vec<f32>, b2: Vec<f32>) -> Net {
        Net { hidden: b1.len(), inputs, outputs, w1, b1, w2, b2 }
    }

    #[test]
    fn test_mlp_row_is_a_distribution() {
        // Outcome: hidden = relu(x5 - 0.5), relu(0.5 - x5); logits favour eat when hungry.
        let mut outcome = net(vec![5], vec![1.0, -1.0], vec![-0.5, 0.5], 5, vec![0.0; 5 * 2], vec![0.0; 5]);
        outcome.w2[1] = 10.0; // eat <- the hungry unit
        let rolls = net(vec![4], vec![1.0], vec![0.0], 8, vec![0.0; 8], vec![-2.0; 8]);
        let m = StatMlp { version: 1, outcome, rolls };
        m.outcome.check("outcome");
        m.rolls.check("rolls");
        let mut x = [0.0f32; N_FEATURES];
        x[5] = 0.0;
        let hungry = m.row_for(&x);
        x[5] = 1.0;
        let fed = m.row_for(&x);
        assert!(hungry.p_eat > 0.9 && fed.p_eat < 0.3, "{} {}", hungry.p_eat, fed.p_eat);
        let total = fed.p_eat + fed.p_work + fed.p_social + fed.p_sleep;
        assert!(total < 1.0 && (fed.p_eat - 0.2).abs() < 1e-5, "{total}");
        let sig = 1.0 / (1.0 + 2.0f32.exp());
        assert!((fed.p_steal - sig).abs() < 1e-6 && (fed.p_chat_home - sig).abs() < 1e-6);
    }
}
