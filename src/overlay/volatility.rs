use std::collections::HashMap;

use chrono::NaiveDate;

use crate::config::VolatilityOverlayConfig;
use crate::core::bar::BarSeries;
use crate::overlay::{OverlayAction, OverlayScope};

// ─── Trigger Results ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TriggerResult {
    pub instrument: String,
    pub vol_ratio: Option<f64>,
    pub atr_pct: Option<f64>,
    pub move_sigma: Option<f64>, // |r_t| / sigma
}

impl TriggerResult {
    fn is_triggered(&self, cfg: &VolatilityOverlayConfig) -> bool {
        self.vol_ratio_triggered(cfg) || self.atr_triggered(cfg) || self.move_triggered(cfg)
    }

    fn is_severe(&self, cfg: &VolatilityOverlayConfig) -> bool {
        if let Some(vr) = self.vol_ratio {
            if vr >= cfg.severe_vol_ratio_threshold {
                return true;
            }
        }
        if let Some(ms) = self.move_sigma {
            if ms >= cfg.severe_move_k {
                return true;
            }
        }
        false
    }

    fn vol_ratio_triggered(&self, cfg: &VolatilityOverlayConfig) -> bool {
        self.vol_ratio
            .is_some_and(|vr| vr >= cfg.vol_ratio_threshold)
    }

    fn atr_triggered(&self, cfg: &VolatilityOverlayConfig) -> bool {
        self.atr_pct
            .is_some_and(|ap| ap >= cfg.atr_pct_threshold)
    }

    fn move_triggered(&self, cfg: &VolatilityOverlayConfig) -> bool {
        self.move_sigma.is_some_and(|ms| ms >= cfg.move_k)
    }
}

// ─── Computation Helpers ────────────────────────────────────────

/// Compute standard deviation of a slice.
fn stdev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    var.sqrt()
}

/// Compute log returns from close prices.
fn log_returns(closes: &[f64]) -> Vec<f64> {
    closes
        .windows(2)
        .map(|w| (w[1] / w[0]).ln())
        .collect()
}

/// Compute ATR using Wilder's smoothing (self-contained, no track-b dependency).
fn compute_atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Option<f64> {
    let n = highs.len().min(lows.len()).min(closes.len());
    if n < period + 1 || period == 0 {
        return None;
    }

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

    let mut atr: f64 = trs[..period].iter().sum::<f64>() / period as f64;
    for &tr in &trs[period..] {
        atr = (atr * (period as f64 - 1.0) + tr) / period as f64;
    }

    Some(atr)
}

// ─── Per-Instrument Trigger Computation ─────────────────────────

fn compute_triggers(
    bars: &BarSeries,
    instrument: &str,
    cfg: &VolatilityOverlayConfig,
) -> TriggerResult {
    let b = bars.bars();

    // Extract close prices
    let closes: Vec<f64> = b.iter().map(|bar| bar.close).collect();
    let returns = log_returns(&closes);

    // A) Realized vol ratio: short / long
    let vol_ratio = if returns.len() >= cfg.vol_long_days {
        let short_window = &returns[returns.len().saturating_sub(cfg.vol_short_days)..];
        let long_window = &returns[returns.len().saturating_sub(cfg.vol_long_days)..];
        let vol_short = stdev(short_window);
        let vol_long = stdev(long_window);
        if vol_long > 0.0 {
            Some(vol_short / vol_long)
        } else {
            None
        }
    } else {
        None
    };

    // B) ATR% = ATR / close
    let atr_pct = if b.len() > cfg.atr_period {
        let highs: Vec<f64> = b.iter().map(|bar| bar.high).collect();
        let lows: Vec<f64> = b.iter().map(|bar| bar.low).collect();
        let last_close = closes.last().copied().unwrap_or(1.0);
        compute_atr(&highs, &lows, &closes, cfg.atr_period).map(|atr| {
            if last_close > 0.0 {
                atr / last_close
            } else {
                0.0
            }
        })
    } else {
        None
    };

    // C) Large move: |r_t| / sigma
    let move_sigma = if !returns.is_empty() && returns.len() >= cfg.sigma_days {
        let sigma_window = &returns[returns.len().saturating_sub(cfg.sigma_days)..];
        let sigma = stdev(sigma_window);
        let last_return = returns.last().copied().unwrap_or(0.0);
        if sigma > 1e-8 {
            Some(last_return.abs() / sigma)
        } else {
            None
        }
    } else {
        None
    };

    TriggerResult {
        instrument: instrument.to_string(),
        vol_ratio,
        atr_pct,
        move_sigma,
    }
}

// ─── Global Action Emission ─────────────────────────────────────

/// Compute volatility overlay actions from bar data.
///
/// Evaluates per-instrument triggers and emits global actions:
/// - Any trigger fires → `ScaleExposure { Global, factor, until }`
/// - Any severe trigger fires → `FreezeEntries { Global, until }`
///
/// Returns the actions and the trigger details for audit logging.
pub fn compute_volatility_actions(
    bars: &HashMap<String, BarSeries>,
    eval_date: NaiveDate,
    cfg: &VolatilityOverlayConfig,
) -> (Vec<OverlayAction>, Vec<TriggerResult>) {
    if !cfg.enabled {
        return (vec![], vec![]);
    }

    let mut triggers: Vec<TriggerResult> = Vec::new();
    let mut any_triggered = false;
    let mut any_severe = false;

    for (sym, series) in bars {
        let result = compute_triggers(series, sym, cfg);
        if result.is_triggered(cfg) {
            any_triggered = true;
        }
        if result.is_severe(cfg) {
            any_severe = true;
        }
        triggers.push(result);
    }

    let until = eval_date + chrono::Duration::days(cfg.until_days as i64);
    let mut actions = Vec::new();

    if any_severe {
        actions.push(OverlayAction::FreezeEntries {
            scope: OverlayScope::Global,
            until,
        });
    } else if any_triggered {
        actions.push(OverlayAction::ScaleExposure {
            scope: OverlayScope::Global,
            factor: cfg.scale_factor,
            until,
        });
    }

    (actions, triggers)
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bar::Bar;

    /// Build a BarSeries from close prices (same OHLC for simplicity).
    fn series_from_closes(closes: &[f64]) -> BarSeries {
        let bars: Vec<Bar> = closes
            .iter()
            .enumerate()
            .map(|(i, &c)| Bar {
                date: NaiveDate::from_ymd_opt(2025, 1, 2).unwrap()
                    + chrono::Duration::days(i as i64),
                open: c,
                high: c,
                low: c,
                close: c,
                volume: 1000.0,
            })
            .collect();
        BarSeries::new(bars).unwrap()
    }

    /// Build a BarSeries with explicit OHLC for ATR testing.
    fn series_with_ohlc(data: &[(f64, f64, f64, f64)]) -> BarSeries {
        let bars: Vec<Bar> = data
            .iter()
            .enumerate()
            .map(|(i, &(o, h, l, c))| Bar {
                date: NaiveDate::from_ymd_opt(2025, 1, 2).unwrap()
                    + chrono::Duration::days(i as i64),
                open: o,
                high: h,
                low: l,
                close: c,
                volume: 1000.0,
            })
            .collect();
        BarSeries::new(bars).unwrap()
    }

    fn default_cfg() -> VolatilityOverlayConfig {
        VolatilityOverlayConfig {
            enabled: true,
            scale_factor: 0.5,
            until_days: 1,
            vol_short_days: 10,
            vol_long_days: 60,
            vol_ratio_threshold: 1.5,
            severe_vol_ratio_threshold: 2.0,
            atr_period: 14,
            atr_pct_threshold: 0.02,
            sigma_days: 60,
            move_k: 1.5,
            severe_move_k: 2.5,
        }
    }

    #[test]
    fn calm_market_no_actions() {
        // 70 days of flat prices — no triggers.
        // Constant prices avoid near-zero stdev producing extreme move_sigma ratios.
        let closes: Vec<f64> = (0..70).map(|_| 100.0).collect();
        let series = series_from_closes(&closes);
        let mut bars = HashMap::new();
        bars.insert("SPY".into(), series);

        let eval = NaiveDate::from_ymd_opt(2025, 3, 13).unwrap();
        let (actions, triggers) = compute_volatility_actions(&bars, eval, &default_cfg());

        assert!(actions.is_empty());
        // Triggers computed but none fired
        assert_eq!(triggers.len(), 1);
        assert!(!triggers[0].is_triggered(&default_cfg()));
    }

    #[test]
    fn vol_ratio_spike_emits_scale() {
        // 60 days calm, then 10 days of moderate vol
        let mut closes: Vec<f64> = (0..60).map(|i| 100.0 + (i as f64) * 0.01).collect();
        // Add 10 days with moderate swings (2% — enough to spike vol ratio)
        for i in 0..10 {
            let last = *closes.last().unwrap();
            let shock = if i % 2 == 0 { 1.02 } else { 0.98 };
            closes.push(last * shock);
        }
        let series = series_from_closes(&closes);
        let mut bars = HashMap::new();
        bars.insert("GLD".into(), series);

        // Raise severe thresholds so moderate swings only trigger normal ScaleExposure
        let mut cfg = default_cfg();
        cfg.severe_vol_ratio_threshold = 100.0;
        cfg.severe_move_k = 100.0;

        let eval = NaiveDate::from_ymd_opt(2025, 3, 13).unwrap();
        let (actions, _triggers) = compute_volatility_actions(&bars, eval, &cfg);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            OverlayAction::ScaleExposure {
                scope,
                factor,
                until,
            } => {
                assert!(matches!(scope, OverlayScope::Global));
                assert!((factor - 0.5).abs() < 1e-10);
                assert_eq!(*until, eval + chrono::Duration::days(1));
            }
            other => panic!("expected ScaleExposure, got {other:?}"),
        }
    }

    #[test]
    fn large_move_emits_scale() {
        // 70 days calm, then final day has a huge move
        let mut closes: Vec<f64> = (0..70).map(|i| 100.0 + (i as f64) * 0.01).collect();
        // Replace last close with a big drop (>1.5σ)
        let last = *closes.last().unwrap();
        *closes.last_mut().unwrap() = last * 0.90; // 10% drop

        let series = series_from_closes(&closes);
        let mut bars = HashMap::new();
        bars.insert("SPY".into(), series);

        let eval = NaiveDate::from_ymd_opt(2025, 3, 13).unwrap();
        let (actions, triggers) = compute_volatility_actions(&bars, eval, &default_cfg());

        assert!(!actions.is_empty());
        // The move should be large enough to trigger
        assert!(triggers[0].move_sigma.unwrap() >= 1.5);
    }

    #[test]
    fn severe_move_emits_freeze() {
        // 70 days calm, then final day has a crash
        let mut closes: Vec<f64> = (0..70).map(|i| 100.0 + (i as f64) * 0.01).collect();
        let last = *closes.last().unwrap();
        *closes.last_mut().unwrap() = last * 0.80; // 20% crash

        let series = series_from_closes(&closes);
        let mut bars = HashMap::new();
        bars.insert("SPY".into(), series);

        let eval = NaiveDate::from_ymd_opt(2025, 3, 13).unwrap();
        let (actions, triggers) = compute_volatility_actions(&bars, eval, &default_cfg());

        assert_eq!(actions.len(), 1);
        assert!(triggers[0].is_severe(&default_cfg()));
        match &actions[0] {
            OverlayAction::FreezeEntries { scope, until } => {
                assert!(matches!(scope, OverlayScope::Global));
                assert_eq!(*until, eval + chrono::Duration::days(1));
            }
            other => panic!("expected FreezeEntries, got {other:?}"),
        }
    }

    #[test]
    fn disabled_config_no_actions() {
        let mut cfg = default_cfg();
        cfg.enabled = false;

        let closes: Vec<f64> = (0..70).map(|_| 100.0).collect();
        let series = series_from_closes(&closes);
        let mut bars = HashMap::new();
        bars.insert("SPY".into(), series);

        let eval = NaiveDate::from_ymd_opt(2025, 3, 13).unwrap();
        let (actions, triggers) = compute_volatility_actions(&bars, eval, &cfg);

        assert!(actions.is_empty());
        assert!(triggers.is_empty());
    }

    #[test]
    fn atr_pct_spike_emits_scale() {
        // Build bars with widening ranges to trigger ATR%
        let n = 30;
        let mut data: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(n);
        for i in 0..n {
            let base = 100.0;
            // Last few bars have very wide ranges
            let range = if i >= n - 5 { 5.0 } else { 0.1 };
            data.push((base, base + range, base - range, base));
        }
        let series = series_with_ohlc(&data);
        let mut bars = HashMap::new();
        bars.insert("GLD".into(), series);

        let mut cfg = default_cfg();
        // Lower threshold so ATR% triggers more easily
        cfg.atr_pct_threshold = 0.01;

        let eval = NaiveDate::from_ymd_opt(2025, 3, 13).unwrap();
        let (actions, triggers) = compute_volatility_actions(&bars, eval, &cfg);

        assert!(!actions.is_empty());
        assert!(triggers[0].atr_triggered(&cfg));
    }

    #[test]
    fn deterministic_same_bars_same_actions() {
        let mut closes: Vec<f64> = (0..60).map(|i| 100.0 + (i as f64) * 0.01).collect();
        for i in 0..10 {
            let last = *closes.last().unwrap();
            closes.push(last * if i % 2 == 0 { 1.05 } else { 0.95 });
        }

        let eval = NaiveDate::from_ymd_opt(2025, 3, 13).unwrap();
        let cfg = default_cfg();

        // Run twice with identical input
        let series1 = series_from_closes(&closes);
        let mut bars1 = HashMap::new();
        bars1.insert("GLD".into(), series1);
        let (actions1, _) = compute_volatility_actions(&bars1, eval, &cfg);

        let series2 = series_from_closes(&closes);
        let mut bars2 = HashMap::new();
        bars2.insert("GLD".into(), series2);
        let (actions2, _) = compute_volatility_actions(&bars2, eval, &cfg);

        assert_eq!(actions1.len(), actions2.len());
    }

    #[test]
    fn insufficient_data_no_triggers() {
        // Only 5 bars — not enough for any trigger
        let closes: Vec<f64> = vec![100.0, 101.0, 99.0, 102.0, 98.0];
        let series = series_from_closes(&closes);
        let mut bars = HashMap::new();
        bars.insert("SPY".into(), series);

        let eval = NaiveDate::from_ymd_opt(2025, 1, 7).unwrap();
        let (actions, triggers) = compute_volatility_actions(&bars, eval, &default_cfg());

        assert!(actions.is_empty());
        // Triggers computed but all None due to insufficient data
        assert!(triggers[0].vol_ratio.is_none());
        assert!(triggers[0].move_sigma.is_none());
    }
}
