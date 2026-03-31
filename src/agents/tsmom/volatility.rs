/// Annualization factor for daily returns: sqrt(252).
const SQRT_252: f64 = 15.874507866387544; // f64::sqrt(252.0)

/// Compute annualized EWMA volatility from daily returns.
///
/// Matches pandas `Series.pow(2).ewm(com=com, min_periods=min_periods).mean()`
/// with `adjust=True` (the pandas default).
///
/// With adjust=True, the EWMA at time t is:
///   ewma_t = (x_t + (1-α)·x_{t-1} + (1-α)²·x_{t-2} + ...) / (1 + (1-α) + (1-α)² + ...)
///
/// This is computed incrementally as:
///   num_t = x_t + (1-α) · num_{t-1}
///   den_t = 1  + (1-α) · den_{t-1}
///   ewma_t = num_t / den_t
///
/// Returns a Vec of annualized volatility values aligned with the input.
/// Values before `min_periods` are set to 0.0.
pub fn ewma_volatility(returns: &[f64], com: usize, min_periods: usize) -> Vec<f64> {
    let alpha = 1.0 / (1.0 + com as f64);
    let decay = 1.0 - alpha;
    let mut result = Vec::with_capacity(returns.len());
    let mut num = 0.0;
    let mut den = 0.0;

    for (i, &r) in returns.iter().enumerate() {
        let x = r * r; // squared return → variance input
        if i == 0 {
            num = x;
            den = 1.0;
        } else {
            num = x + decay * num;
            den = 1.0 + decay * den;
        }

        if i + 1 >= min_periods {
            let ewma_var = num / den;
            result.push(ewma_var.sqrt() * SQRT_252);
        } else {
            result.push(0.0);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ewma_vol_length_matches_input() {
        let returns = vec![0.01, -0.005, 0.008, -0.012, 0.003];
        let vol = ewma_volatility(&returns, 60, 2);
        assert_eq!(vol.len(), returns.len());
    }

    #[test]
    fn ewma_vol_zeros_before_min_periods() {
        let returns = vec![0.01, -0.005, 0.008, -0.012, 0.003];
        let vol = ewma_volatility(&returns, 60, 3);
        assert_eq!(vol[0], 0.0);
        assert_eq!(vol[1], 0.0);
        assert!(vol[2] > 0.0);
    }

    #[test]
    fn ewma_vol_positive_for_nonzero_returns() {
        let returns: Vec<f64> = (0..100).map(|i| if i % 2 == 0 { 0.01 } else { -0.01 }).collect();
        let vol = ewma_volatility(&returns, 60, 20);
        // After min_periods, all should be positive
        for &v in &vol[20..] {
            assert!(v > 0.0);
        }
    }

    #[test]
    fn ewma_vol_constant_returns_converges() {
        // With constant absolute returns, EWMA variance should converge to r^2
        let r = 0.01;
        let returns: Vec<f64> = vec![r; 500];
        let vol = ewma_volatility(&returns, 60, 20);
        let expected_daily_vol = r; // sqrt(r^2) = |r|
        let expected_ann_vol = expected_daily_vol * SQRT_252;
        // Last value should be very close to converged
        assert!((vol[499] - expected_ann_vol).abs() / expected_ann_vol < 0.01);
    }
}
