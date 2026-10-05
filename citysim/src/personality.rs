//! Personality: six axes in `0..=1`, their initial distributions and the
//! trait-drift table applied by whichever system raises the event.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::components::Personality;

/// Median of three independent `U(0,1)` draws, i.e. `Beta(2,2)`.
fn beta22(rng: &mut ChaCha8Rng) -> f32 {
    let mut d = [rng.random::<f32>(), rng.random::<f32>(), rng.random::<f32>()];
    d.sort_by(f32::total_cmp);
    d[1]
}

/// An event that drifts personality traits (spec › Personality table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drift {
    Arrested,
    ServedFullSentence,
    StarvingDay,
    CrimeUnpunished,
    JoinedGang,
    LeftGang,
    WonFight,
    LostFight,
    Robbed,
    Married,
    SpouseDied,
    WageUnpaidDay,
    PaidOnTimeWeek,
    ReportedCrime,
    BirthdayAfter40,
    /// M9: the captain took a gang's money.
    TookBribe,
}

impl Personality {
    /// Initial population: every axis `Beta(2,2)`.
    pub fn random(rng: &mut ChaCha8Rng) -> Personality {
        Personality {
            lawfulness: beta22(rng),
            greed: beta22(rng),
            pride: beta22(rng),
            sociability: beta22(rng),
            courage: beta22(rng),
            loyalty: beta22(rng),
        }
    }

    /// A child: `clamp((mother + father) / 2 + N(0, 0.15), 0, 1)` per axis;
    /// an unknown parent contributes 0.5.
    pub fn inherit(mother: Option<&Personality>, father: Option<&Personality>, rng: &mut ChaCha8Rng) -> Personality {
        let mut axis = |f: fn(&Personality) -> f32| {
            let m = mother.map_or(0.5, f);
            let d = father.map_or(0.5, f);
            // Box–Muller for N(0, 0.15)
            let u1: f32 = rng.random::<f32>().max(1e-6);
            let u2: f32 = rng.random::<f32>();
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
            ((m + d) / 2.0 + 0.15 * z).clamp(0.0, 1.0)
        };
        Personality {
            lawfulness: axis(|p| p.lawfulness),
            greed: axis(|p| p.greed),
            pride: axis(|p| p.pride),
            sociability: axis(|p| p.sociability),
            courage: axis(|p| p.courage),
            loyalty: axis(|p| p.loyalty),
        }
    }

    /// Apply one row of the drift table, clamped to `0..=1`.
    pub fn drift(&mut self, event: Drift) {
        let add = |v: &mut f32, d: f32| *v = (*v + d).clamp(0.0, 1.0);
        match event {
            Drift::Arrested => {
                add(&mut self.lawfulness, -0.05);
                add(&mut self.pride, -0.03);
            }
            Drift::ServedFullSentence => add(&mut self.lawfulness, 0.02),
            Drift::StarvingDay => {
                add(&mut self.lawfulness, -0.02);
                add(&mut self.courage, 0.01);
            }
            Drift::CrimeUnpunished => {
                add(&mut self.lawfulness, -0.03);
                add(&mut self.greed, 0.02);
            }
            Drift::JoinedGang => {
                add(&mut self.loyalty, 0.10);
                add(&mut self.lawfulness, -0.10);
            }
            Drift::LeftGang => add(&mut self.loyalty, -0.15),
            Drift::WonFight => {
                add(&mut self.courage, 0.03);
                add(&mut self.pride, 0.02);
            }
            Drift::LostFight => add(&mut self.courage, -0.05),
            Drift::Robbed => {
                add(&mut self.lawfulness, 0.02);
                add(&mut self.courage, -0.02);
            }
            Drift::Married => {
                add(&mut self.loyalty, 0.05);
                add(&mut self.sociability, 0.02);
            }
            Drift::SpouseDied => add(&mut self.sociability, -0.05),
            Drift::WageUnpaidDay => {
                add(&mut self.lawfulness, -0.01);
                add(&mut self.loyalty, -0.02);
            }
            Drift::PaidOnTimeWeek => add(&mut self.lawfulness, 0.01),
            Drift::ReportedCrime => add(&mut self.lawfulness, 0.01),
            Drift::BirthdayAfter40 => {
                add(&mut self.greed, -0.005);
                add(&mut self.courage, -0.005);
            }
            Drift::TookBribe => {
                add(&mut self.lawfulness, -0.05);
                add(&mut self.greed, 0.02);
            }
        }
    }
}
