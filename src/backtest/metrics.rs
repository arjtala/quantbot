use super::engine::Snapshot;

const TRADING_DAYS_PER_YEAR: f64 = 252.0;

/// Computed performance metrics from backtest snapshots.
#[derive(Debug, Clone)]
pub struct BacktestResult {
    pub sharpe_ratio: f64,
    pub annualized_return: f64,
    pub annualized_vol: f64,
    pub max_drawdown: f64,
    pub max_drawdown_duration_days: usize,
    pub calmar_ratio: f64,
    pub sortino_ratio: f64,
    pub total_return: f64,
    pub total_trades: usize,
    pub nav_series: Vec<(chrono::NaiveDate, f64)>,
}

impl BacktestResult {
    /// Compute metrics from a list of backtest snapshots.
    pub fn from_snapshots(snapshots: &[Snapshot]) -> Option<Self> {
        if snapshots.len() < 2 {
            return None;
        }

        let navs: Vec<f64> = snapshots.iter().map(|s| s.nav).collect();
        let nav_series: Vec<(chrono::NaiveDate, f64)> =
            snapshots.iter().map(|s| (s.date, s.nav)).collect();

        // Daily returns
        let daily_returns: Vec<f64> = navs.windows(2).map(|w| w[1] / w[0] - 1.0).collect();

        let total_trades: usize = snapshots.iter().map(|s| s.fills.len()).sum();

        // Total and annualized return
        let total_return = navs.last().unwrap() / navs[0] - 1.0;
        let n_years = daily_returns.len() as f64 / TRADING_DAYS_PER_YEAR;
        let ann_return = if n_years > 0.0 {
            (1.0 + total_return).powf(1.0 / n_years) - 1.0
        } else {
            0.0
        };

        // Annualized volatility
        let ann_vol = std_dev(&daily_returns) * TRADING_DAYS_PER_YEAR.sqrt();

        // Sharpe ratio (0% risk-free)
        let sharpe = if ann_vol > 1e-8 {
            ann_return / ann_vol
        } else {
            0.0
        };

        // Max drawdown
        let mut peak = navs[0];
        let mut max_dd: f64 = 0.0;
        let mut dd_duration = 0usize;
        let mut max_dd_duration = 0usize;

        for &nav in &navs[1..] {
            if nav > peak {
                peak = nav;
                dd_duration = 0;
            } else {
                let dd = (nav - peak) / peak;
                max_dd = max_dd.min(dd);
                dd_duration += 1;
                max_dd_duration = max_dd_duration.max(dd_duration);
            }
        }

        // Calmar ratio
        let calmar = if max_dd.abs() > 1e-8 {
            ann_return / max_dd.abs()
        } else {
            0.0
        };

        // Sortino ratio (downside deviation)
        let downside: Vec<f64> = daily_returns.iter().filter(|&&r| r < 0.0).copied().collect();
        let downside_std = if downside.is_empty() {
            1e-8
        } else {
            std_dev(&downside) * TRADING_DAYS_PER_YEAR.sqrt()
        };
        let sortino = if downside_std > 1e-8 {
            ann_return / downside_std
        } else {
            0.0
        };

        Some(Self {
            sharpe_ratio: sharpe,
            annualized_return: ann_return,
            annualized_vol: ann_vol,
            max_drawdown: max_dd,
            max_drawdown_duration_days: max_dd_duration,
            calmar_ratio: calmar,
            sortino_ratio: sortino,
            total_return,
            total_trades,
            nav_series,
        })
    }

    /// Human-readable performance summary.
    pub fn summary(&self) -> String {
        let (start_date, start_nav) = self.nav_series.first().unwrap();
        let (end_date, end_nav) = self.nav_series.last().unwrap();
        format!(
            "\
==================================================
  BACKTEST RESULTS
==================================================
  Period:          {start_date} → {end_date}
  Starting NAV:    ${start_nav:.0}
  Ending NAV:      ${end_nav:.0}
  Total Return:    {total:.1}%
--------------------------------------------------
  Ann. Return:     {ann_ret:.2}%
  Ann. Volatility: {ann_vol:.2}%
  Sharpe Ratio:    {sharpe:.2}
  Sortino Ratio:   {sortino:.2}
  Calmar Ratio:    {calmar:.2}
  Max Drawdown:    {max_dd:.2}%
  Max DD Duration: {dd_dur} days
  Total Trades:    {trades}
==================================================",
            total = self.total_return * 100.0,
            ann_ret = self.annualized_return * 100.0,
            ann_vol = self.annualized_vol * 100.0,
            sharpe = self.sharpe_ratio,
            sortino = self.sortino_ratio,
            calmar = self.calmar_ratio,
            max_dd = self.max_drawdown * 100.0,
            dd_dur = self.max_drawdown_duration_days,
            trades = self.total_trades,
        )
    }
}

/// Sample standard deviation.
fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    var.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_snapshots(navs: &[f64]) -> Vec<Snapshot> {
        let base = chrono::NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
        navs.iter()
            .enumerate()
            .map(|(i, &nav)| Snapshot {
                date: base + chrono::Days::new(i as u64),
                nav,
                cash: nav,
                gross_exposure: 0.0,
                net_exposure: 0.0,
                positions: HashMap::new(),
                signals: HashMap::new(),
                fills: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn sharpe_positive_for_uptrend() {
        // Uptrend with alternating daily returns so std_dev > 0
        let mut navs = vec![1_000_000.0];
        for i in 1..253 {
            let ret = if i % 2 == 0 { 0.002 } else { 0.001 };
            navs.push(navs[i - 1] * (1.0 + ret));
        }
        let snaps = make_snapshots(&navs);
        let result = BacktestResult::from_snapshots(&snaps).unwrap();
        assert!(result.sharpe_ratio > 0.0);
        assert!(result.annualized_return > 0.0);
        assert!(result.max_drawdown > -0.01);
    }

    #[test]
    fn sharpe_negative_for_downtrend() {
        // Downtrend with alternating daily returns so std_dev > 0
        let mut navs = vec![1_000_000.0];
        for i in 1..253 {
            let ret = if i % 2 == 0 { -0.002 } else { -0.001 };
            navs.push(navs[i - 1] * (1.0 + ret));
        }
        let snaps = make_snapshots(&navs);
        let result = BacktestResult::from_snapshots(&snaps).unwrap();
        assert!(result.sharpe_ratio < 0.0);
        assert!(result.max_drawdown < 0.0);
    }

    #[test]
    fn max_drawdown_computed_correctly() {
        // NAV goes 100 → 120 → 90 → 110
        let snaps = make_snapshots(&[100.0, 120.0, 90.0, 110.0]);
        let result = BacktestResult::from_snapshots(&snaps).unwrap();
        // Max DD = (90 - 120) / 120 = -25%
        assert!((result.max_drawdown - (-0.25)).abs() < 1e-10);
    }

    #[test]
    fn too_few_snapshots_returns_none() {
        let snaps = make_snapshots(&[100.0]);
        assert!(BacktestResult::from_snapshots(&snaps).is_none());
    }

    #[test]
    fn summary_contains_key_metrics() {
        let navs: Vec<f64> = (0..100).map(|i| 1_000_000.0 * 1.001_f64.powi(i)).collect();
        let snaps = make_snapshots(&navs);
        let result = BacktestResult::from_snapshots(&snaps).unwrap();
        let text = result.summary();
        assert!(text.contains("Sharpe Ratio"));
        assert!(text.contains("Max Drawdown"));
        assert!(text.contains("Total Trades"));
    }

    #[test]
    fn std_dev_correct() {
        let vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = super::std_dev(&vals);
        // Sample std dev: sqrt(32/7) ≈ 2.138
        assert!((sd - 2.138).abs() < 0.01);
    }
}
