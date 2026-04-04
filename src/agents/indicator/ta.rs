use crate::core::bar::BarSeries;

/// MACD computation result.
#[derive(Debug, Clone, Copy)]
pub struct MacdResult {
    pub macd_line: f64,
    pub signal_line: f64,
    pub histogram: f64,
}

/// Bollinger Bands computation result.
#[derive(Debug, Clone, Copy)]
pub struct BollingerResult {
    pub upper: f64,
    pub middle: f64,
    pub lower: f64,
    pub bandwidth: f64,
}

/// Snapshot of all TA indicators for a single instrument.
#[derive(Debug, Clone)]
pub struct TaSnapshot {
    pub rsi_14: Option<f64>,
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub ema_12: Option<f64>,
    pub ema_26: Option<f64>,
    pub macd: Option<MacdResult>,
    pub bollinger: Option<BollingerResult>,
    pub atr_14: Option<f64>,
    pub last_close: f64,
}

impl TaSnapshot {
    /// Compute all TA indicators from a bar series.
    pub fn compute(bars: &BarSeries) -> Self {
        let data = bars.bars();
        let closes: Vec<f64> = data.iter().map(|b| b.close).collect();
        let highs: Vec<f64> = data.iter().map(|b| b.high).collect();
        let lows: Vec<f64> = data.iter().map(|b| b.low).collect();
        let last_close = closes.last().copied().unwrap_or(0.0);

        Self {
            rsi_14: if closes.len() > 14 {
                Some(compute_rsi(&closes, 14))
            } else {
                None
            },
            sma_20: compute_sma(&closes, 20),
            sma_50: compute_sma(&closes, 50),
            ema_12: compute_ema(&closes, 12),
            ema_26: compute_ema(&closes, 26),
            macd: compute_macd(&closes, 12, 26, 9),
            bollinger: compute_bollinger(&closes, 20, 2.0),
            atr_14: compute_atr(&highs, &lows, &closes, 14),
            last_close,
        }
    }

    /// Format TA snapshot as a human-readable string for the LLM prompt.
    pub fn format_for_prompt(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Last Close: {:.4}", self.last_close));

        if let Some(rsi) = self.rsi_14 {
            lines.push(format!("RSI(14): {:.1}", rsi));
        }
        if let Some(sma) = self.sma_20 {
            lines.push(format!("SMA(20): {:.4}", sma));
        }
        if let Some(sma) = self.sma_50 {
            lines.push(format!("SMA(50): {:.4}", sma));
        }
        if let Some(ema) = self.ema_12 {
            lines.push(format!("EMA(12): {:.4}", ema));
        }
        if let Some(ema) = self.ema_26 {
            lines.push(format!("EMA(26): {:.4}", ema));
        }
        if let Some(macd) = &self.macd {
            lines.push(format!(
                "MACD: line={:.4}, signal={:.4}, histogram={:.4}",
                macd.macd_line, macd.signal_line, macd.histogram
            ));
        }
        if let Some(bb) = &self.bollinger {
            lines.push(format!(
                "Bollinger(20,2): upper={:.4}, middle={:.4}, lower={:.4}, bandwidth={:.4}",
                bb.upper, bb.middle, bb.lower, bb.bandwidth
            ));
        }
        if let Some(atr) = self.atr_14 {
            lines.push(format!("ATR(14): {:.4}", atr));
        }

        lines.join("\n")
    }
}

/// Compute RSI using Wilder's smoothing (exponential moving average of gains/losses).
pub fn compute_rsi(closes: &[f64], period: usize) -> f64 {
    debug_assert!(closes.len() > period);

    let changes: Vec<f64> = closes.windows(2).map(|w| w[1] - w[0]).collect();

    // Initial average gain/loss from first `period` changes
    let mut avg_gain = 0.0;
    let mut avg_loss = 0.0;
    for &change in &changes[..period] {
        if change > 0.0 {
            avg_gain += change;
        } else {
            avg_loss += -change;
        }
    }
    avg_gain /= period as f64;
    avg_loss /= period as f64;

    // Wilder's smoothing for remaining changes
    for &change in &changes[period..] {
        if change > 0.0 {
            avg_gain = (avg_gain * (period as f64 - 1.0) + change) / period as f64;
            avg_loss = (avg_loss * (period as f64 - 1.0)) / period as f64;
        } else {
            avg_gain = (avg_gain * (period as f64 - 1.0)) / period as f64;
            avg_loss = (avg_loss * (period as f64 - 1.0) + (-change)) / period as f64;
        }
    }

    if avg_loss < 1e-14 {
        return 100.0;
    }

    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

/// Simple Moving Average over the last `period` values.
pub fn compute_sma(values: &[f64], period: usize) -> Option<f64> {
    if values.len() < period || period == 0 {
        return None;
    }
    let sum: f64 = values[values.len() - period..].iter().sum();
    Some(sum / period as f64)
}

/// Exponential Moving Average over the full series.
pub fn compute_ema(values: &[f64], period: usize) -> Option<f64> {
    if values.len() < period || period == 0 {
        return None;
    }
    // Seed with SMA of first `period` values
    let sma: f64 = values[..period].iter().sum::<f64>() / period as f64;
    let multiplier = 2.0 / (period as f64 + 1.0);

    let mut ema = sma;
    for &v in &values[period..] {
        ema = (v - ema) * multiplier + ema;
    }
    Some(ema)
}

/// MACD: difference between fast and slow EMA, with a signal line (EMA of MACD line).
pub fn compute_macd(
    closes: &[f64],
    fast: usize,
    slow: usize,
    signal: usize,
) -> Option<MacdResult> {
    if closes.len() < slow + signal {
        return None;
    }

    // Compute fast and slow EMAs for the full series to build a MACD line series
    let fast_sma: f64 = closes[..fast].iter().sum::<f64>() / fast as f64;
    let slow_sma: f64 = closes[..slow].iter().sum::<f64>() / slow as f64;
    let fast_mult = 2.0 / (fast as f64 + 1.0);
    let slow_mult = 2.0 / (slow as f64 + 1.0);

    let mut fast_ema = fast_sma;
    let mut slow_ema = slow_sma;

    // Build MACD line values starting from index `slow` (where slow EMA is seeded)
    let mut macd_values = Vec::new();

    // Update EMAs through the series, collecting MACD values once slow EMA is seeded
    for (i, &c) in closes.iter().enumerate().skip(1) {
        if i < fast {
            // Still building fast SMA seed — skip
            continue;
        }
        if i == fast {
            // Fast EMA is seeded, start updating
            fast_ema = (c - fast_ema) * fast_mult + fast_ema;
            if i >= slow {
                slow_ema = (c - slow_ema) * slow_mult + slow_ema;
                macd_values.push(fast_ema - slow_ema);
            }
            continue;
        }

        fast_ema = (c - fast_ema) * fast_mult + fast_ema;
        if i < slow {
            continue;
        }
        slow_ema = (c - slow_ema) * slow_mult + slow_ema;
        macd_values.push(fast_ema - slow_ema);
    }

    if macd_values.len() < signal {
        return None;
    }

    // Signal line = EMA of MACD values
    let signal_sma: f64 = macd_values[..signal].iter().sum::<f64>() / signal as f64;
    let signal_mult = 2.0 / (signal as f64 + 1.0);
    let mut signal_ema = signal_sma;
    for &v in &macd_values[signal..] {
        signal_ema = (v - signal_ema) * signal_mult + signal_ema;
    }

    let macd_line = *macd_values.last().unwrap();
    Some(MacdResult {
        macd_line,
        signal_line: signal_ema,
        histogram: macd_line - signal_ema,
    })
}

/// Bollinger Bands: middle = SMA, upper/lower = middle ± num_std × stddev.
pub fn compute_bollinger(closes: &[f64], period: usize, num_std: f64) -> Option<BollingerResult> {
    if closes.len() < period || period == 0 {
        return None;
    }

    let window = &closes[closes.len() - period..];
    let middle = window.iter().sum::<f64>() / period as f64;
    let variance = window.iter().map(|&x| (x - middle).powi(2)).sum::<f64>() / period as f64;
    let std_dev = variance.sqrt();

    let upper = middle + num_std * std_dev;
    let lower = middle - num_std * std_dev;
    let bandwidth = if middle.abs() > 1e-14 {
        (upper - lower) / middle
    } else {
        0.0
    };

    Some(BollingerResult {
        upper,
        middle,
        lower,
        bandwidth,
    })
}

/// Average True Range using Wilder's smoothing.
pub fn compute_atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Option<f64> {
    let n = highs.len().min(lows.len()).min(closes.len());
    if n < period + 1 || period == 0 {
        return None;
    }

    // True Range for each bar (starting from index 1)
    let mut trs = Vec::with_capacity(n - 1);
    for i in 1..n {
        let hl = highs[i] - lows[i];
        let hc = (highs[i] - closes[i - 1]).abs();
        let lc = (lows[i] - closes[i - 1]).abs();
        trs.push(hl.max(hc).max(lc));
    }

    if trs.len() < period {
        return None;
    }

    // Initial ATR = simple average of first `period` TRs
    let mut atr: f64 = trs[..period].iter().sum::<f64>() / period as f64;

    // Wilder's smoothing
    for &tr in &trs[period..] {
        atr = (atr * (period as f64 - 1.0) + tr) / period as f64;
    }

    Some(atr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bar::Bar;
    use chrono::NaiveDate;

    fn make_bars(prices: &[f64]) -> BarSeries {
        let base_date = NaiveDate::from_ymd_opt(2023, 1, 2).unwrap();
        let bars: Vec<Bar> = prices
            .iter()
            .enumerate()
            .map(|(i, &price)| Bar {
                date: base_date + chrono::Days::new(i as u64),
                open: price,
                high: price * 1.02,
                low: price * 0.98,
                close: price,
                volume: 10000.0,
            })
            .collect();
        BarSeries::new(bars).unwrap()
    }

    fn make_ohlc_bars(data: &[(f64, f64, f64, f64)]) -> BarSeries {
        let base_date = NaiveDate::from_ymd_opt(2023, 1, 2).unwrap();
        let bars: Vec<Bar> = data
            .iter()
            .enumerate()
            .map(|(i, &(o, h, l, c))| Bar {
                date: base_date + chrono::Days::new(i as u64),
                open: o,
                high: h,
                low: l,
                close: c,
                volume: 10000.0,
            })
            .collect();
        BarSeries::new(bars).unwrap()
    }

    // ── RSI tests ────────────────────────────────────────────────

    #[test]
    fn rsi_strong_downtrend() {
        let mut prices = vec![100.0];
        for i in 1..30 {
            prices.push(100.0 - i as f64 * 1.5);
        }
        let rsi = compute_rsi(&prices, 14);
        assert!(rsi < 30.0, "RSI should be < 30 in downtrend, got {rsi}");
    }

    #[test]
    fn rsi_strong_uptrend() {
        let mut prices = vec![100.0];
        for i in 1..30 {
            prices.push(100.0 + i as f64 * 1.5);
        }
        let rsi = compute_rsi(&prices, 14);
        assert!(rsi > 70.0, "RSI should be > 70 in uptrend, got {rsi}");
    }

    #[test]
    fn rsi_all_gains_returns_100() {
        let prices: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        let rsi = compute_rsi(&prices, 14);
        assert!((rsi - 100.0).abs() < 1e-10);
    }

    // ── SMA tests ────────────────────────────────────────────────

    #[test]
    fn sma_basic() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((compute_sma(&values, 3).unwrap() - 4.0).abs() < 1e-10);
        assert!((compute_sma(&values, 5).unwrap() - 3.0).abs() < 1e-10);
    }

    #[test]
    fn sma_insufficient_data() {
        assert!(compute_sma(&[1.0, 2.0], 5).is_none());
    }

    // ── EMA tests ────────────────────────────────────────────────

    #[test]
    fn ema_basic() {
        // Exponentially increasing series — EMA weights recent (larger) values more
        let values: Vec<f64> = (1..=20).map(|i| (i as f64).powi(2)).collect();
        let ema = compute_ema(&values, 10).unwrap();
        let sma = compute_sma(&values, 10).unwrap();
        assert!(ema > sma, "EMA ({ema}) should weight recent values more than SMA ({sma})");
    }

    #[test]
    fn ema_insufficient_data() {
        assert!(compute_ema(&[1.0, 2.0], 5).is_none());
    }

    // ── MACD tests ───────────────────────────────────────────────

    #[test]
    fn macd_uptrend() {
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let macd = compute_macd(&prices, 12, 26, 9).unwrap();
        assert!(macd.macd_line > 0.0, "MACD line should be positive in uptrend");
    }

    #[test]
    fn macd_insufficient_data() {
        let prices: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        assert!(compute_macd(&prices, 12, 26, 9).is_none());
    }

    // ── Bollinger tests ──────────────────────────────────────────

    #[test]
    fn bollinger_basic() {
        let prices: Vec<f64> = (0..30).map(|i| 100.0 + (i as f64 * 0.1).sin()).collect();
        let bb = compute_bollinger(&prices, 20, 2.0).unwrap();
        assert!(bb.upper > bb.middle);
        assert!(bb.lower < bb.middle);
        assert!(bb.bandwidth > 0.0);
    }

    #[test]
    fn bollinger_constant_prices() {
        let prices = vec![100.0; 25];
        let bb = compute_bollinger(&prices, 20, 2.0).unwrap();
        assert!((bb.upper - 100.0).abs() < 1e-10);
        assert!((bb.lower - 100.0).abs() < 1e-10);
        assert!(bb.bandwidth.abs() < 1e-10);
    }

    #[test]
    fn bollinger_insufficient_data() {
        assert!(compute_bollinger(&[1.0, 2.0], 20, 2.0).is_none());
    }

    // ── ATR tests ────────────────────────────────────────────────

    #[test]
    fn atr_basic() {
        // Construct bars with known high-low ranges
        let data: Vec<(f64, f64, f64, f64)> = (0..30)
            .map(|i| {
                let c = 100.0 + i as f64 * 0.5;
                (c, c + 2.0, c - 2.0, c)
            })
            .collect();
        let bars = make_ohlc_bars(&data);
        let highs: Vec<f64> = bars.bars().iter().map(|b| b.high).collect();
        let lows: Vec<f64> = bars.bars().iter().map(|b| b.low).collect();
        let closes: Vec<f64> = bars.bars().iter().map(|b| b.close).collect();
        let atr = compute_atr(&highs, &lows, &closes, 14).unwrap();
        assert!(atr > 0.0, "ATR should be positive");
        // High-low range is 4.0, plus gaps of 0.5 → TR ≈ 4.5
        assert!(atr > 3.0 && atr < 6.0, "ATR should be around 4-5, got {atr}");
    }

    #[test]
    fn atr_insufficient_data() {
        assert!(compute_atr(&[1.0, 2.0], &[0.5, 1.5], &[0.8, 1.8], 14).is_none());
    }

    // ── TaSnapshot tests ─────────────────────────────────────────

    #[test]
    fn ta_snapshot_compute() {
        let mut prices = vec![100.0];
        for i in 1..60 {
            prices.push(100.0 + (i as f64 * 0.3).sin() * 5.0);
        }
        let bars = make_bars(&prices);
        let snap = TaSnapshot::compute(&bars);

        assert!(snap.rsi_14.is_some());
        assert!(snap.sma_20.is_some());
        assert!(snap.sma_50.is_some());
        assert!(snap.ema_12.is_some());
        assert!(snap.ema_26.is_some());
        assert!(snap.macd.is_some());
        assert!(snap.bollinger.is_some());
        assert!(snap.atr_14.is_some());
    }

    #[test]
    fn ta_snapshot_insufficient_data() {
        let bars = make_bars(&[100.0, 101.0, 102.0]);
        let snap = TaSnapshot::compute(&bars);

        assert!(snap.rsi_14.is_none());
        assert!(snap.sma_20.is_none());
        assert!(snap.sma_50.is_none());
        assert!(snap.macd.is_none());
    }

    #[test]
    fn ta_snapshot_format_for_prompt() {
        let mut prices = vec![100.0];
        for i in 1..60 {
            prices.push(100.0 + i as f64 * 0.5);
        }
        let bars = make_bars(&prices);
        let snap = TaSnapshot::compute(&bars);
        let text = snap.format_for_prompt();

        assert!(text.contains("Last Close:"));
        assert!(text.contains("RSI(14):"));
        assert!(text.contains("SMA(20):"));
        assert!(text.contains("MACD:"));
        assert!(text.contains("Bollinger(20,2):"));
        assert!(text.contains("ATR(14):"));
    }
}
