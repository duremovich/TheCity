//! Small shared helpers with no world state.

/// Deal `n` whole units over `weights` by the largest-remainder method
/// (plan D10, shared with sanitation D23): every slot first gets the floor of
/// its exact share, then the units left go one each by remainder, largest
/// first, ties to the lower index. Negative and non-finite weights count as
/// zero; all-zero weights give all zeros (nothing is dealt). The result sums
/// to `n` otherwise. Slots saturate at `u8::MAX`.
pub fn largest_remainder(n: u32, weights: &[f32]) -> Vec<u8> {
    let w: Vec<f64> = weights.iter().map(|&x| if x.is_finite() && x > 0.0 { f64::from(x) } else { 0.0 }).collect();
    let total: f64 = w.iter().sum();
    let mut out = vec![0u8; w.len()];
    if total <= 0.0 || n == 0 {
        return out;
    }
    let exact: Vec<f64> = w.iter().map(|&x| x / total * f64::from(n)).collect();
    let mut dealt = 0u32;
    for (o, &e) in out.iter_mut().zip(&exact) {
        let f = e.floor() as u32;
        *o = f.min(u32::from(u8::MAX)) as u8;
        dealt += f;
    }
    let mut order: Vec<usize> = (0..w.len()).filter(|&i| w[i] > 0.0).collect();
    order.sort_by(|&a, &b| {
        let (ra, rb) = (exact[a] - exact[a].floor(), exact[b] - exact[b].floor());
        rb.total_cmp(&ra).then(a.cmp(&b))
    });
    for &i in order.iter().cycle().take(n.saturating_sub(dealt) as usize) {
        out[i] = out[i].saturating_add(1);
    }
    out
}
