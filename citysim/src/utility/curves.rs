//! Response curves. Inputs are normalised to `x ∈ [0, 1]`; outputs are
//! clamped to `[0, 1]`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    /// `y = m*x + b`
    Linear { m: f32, b: f32 },
    /// `y = m*(x − c)^k + b`
    Quadratic { k: f32, m: f32, c: f32, b: f32 },
    /// `y = 1 / (1 + e^(−k*(x − mid)))`
    Logistic { k: f32, mid: f32 },
    /// `y = x >= t ? hi : lo`
    Step { t: f32, lo: f32, hi: f32 },
}

impl Curve {
    pub fn eval(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        let y = match self {
            Curve::Linear { m, b } => m * x + b,
            Curve::Quadratic { k, m, c, b } => m * (x - c).powf(k) + b,
            Curve::Logistic { k, mid } => 1.0 / (1.0 + (-k * (x - mid)).exp()),
            Curve::Step { t, lo, hi } => {
                if x >= t {
                    hi
                } else {
                    lo
                }
            }
        };
        if y.is_nan() {
            0.0
        } else {
            y.clamp(0.0, 1.0)
        }
    }
}

/// `Step{1, 0, 1}`: a hard gate.
pub const GATE: Curve = Curve::Step { t: 1.0, lo: 0.0, hi: 1.0 };
/// `Linear{1, 0}`: identity.
pub const IDENTITY: Curve = Curve::Linear { m: 1.0, b: 0.0 };
/// `Quadratic{2, 1, 0, 0}`: `x²`.
pub const SQUARE: Curve = Curve::Quadratic { k: 2.0, m: 1.0, c: 0.0, b: 0.0 };

/// A soft gate: `Step{1, lo, 1}`.
pub const fn gate_or(lo: f32) -> Curve {
    Curve::Step { t: 1.0, lo, hi: 1.0 }
}

/// `Can(cond)` as a curve input.
pub fn can(cond: bool) -> f32 {
    if cond {
        1.0
    } else {
        0.0
    }
}

/// Urgency of a need: `1 − need`.
pub fn urgency(need: f32) -> f32 {
    (1.0 - need).clamp(0.0, 1.0)
}
