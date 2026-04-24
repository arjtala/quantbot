use std::path::Path;

use serde::{Deserialize, Serialize};

use super::engine::Snapshot;

const TRADING_DAYS_PER_YEAR: f64 = 252.0;

/// Computed performance metrics from backtest snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub sharpe_ratio: f64,
    pub annualized_return: f64,
    pub annualized_vol: f64,
    pub max_drawdown: f64,
    pub max_drawdown_duration_days: usize,
    pub calmar_ratio: f64,
    pub sortino_ratio: f64,
    pub var_95: f64,
    pub cvar_95: f64,
    pub var_99: f64,
    pub cvar_99: f64,
    pub rolling_sharpe_mean_63d: f64,
    pub rolling_sharpe_min_63d: f64,
    pub rolling_sharpe_max_63d: f64,
    pub rolling_sharpe_std_63d: f64,
    pub rolling_sharpe_positive_pct_63d: f64,
    pub avg_daily_turnover: f64,
    pub total_return: f64,
    pub total_trades: usize,
    pub nav_series: Vec<(chrono::NaiveDate, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricComparisonRow {
    pub metric: String,
    pub baseline: f64,
    pub candidate: f64,
    pub delta: f64,
    pub delta_pct: f64,
    pub preferred_direction: String,
    pub assessment: String,
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
        let downside: Vec<f64> = daily_returns
            .iter()
            .filter(|&&r| r < 0.0)
            .copied()
            .collect();
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

        // Historical daily VaR / CVaR
        let (var_95, cvar_95) = historical_var_cvar(&daily_returns, 0.95);
        let (var_99, cvar_99) = historical_var_cvar(&daily_returns, 0.99);

        // Rolling 63-day Sharpe stability
        let rolling_sharpes_63d = rolling_sharpes(&daily_returns, 63);
        let rolling_sharpe_mean_63d = mean(&rolling_sharpes_63d);
        let rolling_sharpe_min_63d = rolling_sharpes_63d
            .iter()
            .copied()
            .reduce(f64::min)
            .unwrap_or(0.0);
        let rolling_sharpe_max_63d = rolling_sharpes_63d
            .iter()
            .copied()
            .reduce(f64::max)
            .unwrap_or(0.0);
        let rolling_sharpe_std_63d = std_dev(&rolling_sharpes_63d);
        let rolling_sharpe_positive_pct_63d = if rolling_sharpes_63d.is_empty() {
            0.0
        } else {
            rolling_sharpes_63d.iter().filter(|&&s| s > 0.0).count() as f64
                / rolling_sharpes_63d.len() as f64
        };

        let avg_daily_turnover = average_daily_turnover(snapshots);

        Some(Self {
            sharpe_ratio: sharpe,
            annualized_return: ann_return,
            annualized_vol: ann_vol,
            max_drawdown: max_dd,
            max_drawdown_duration_days: max_dd_duration,
            calmar_ratio: calmar,
            sortino_ratio: sortino,
            var_95,
            cvar_95,
            var_99,
            cvar_99,
            rolling_sharpe_mean_63d,
            rolling_sharpe_min_63d,
            rolling_sharpe_max_63d,
            rolling_sharpe_std_63d,
            rolling_sharpe_positive_pct_63d,
            avg_daily_turnover,
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
--------------------------------------------------
  RISK & ROBUSTNESS
--------------------------------------------------
  Daily VaR 95%:   {var_95:.2}%
  Daily CVaR 95%:  {cvar_95:.2}%
  Daily VaR 99%:   {var_99:.2}%
  Daily CVaR 99%:  {cvar_99:.2}%
  Roll Sharpe 63d: mean {roll_mean:.2} | min {roll_min:.2} | max {roll_max:.2}
  Roll Sharpe 63d std: {roll_std:.2}
  Roll Sharpe 63d > 0: {roll_pos:.1}%
  Avg Daily Turn:  {turnover:.2}%
--------------------------------------------------
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
            var_95 = self.var_95 * 100.0,
            cvar_95 = self.cvar_95 * 100.0,
            var_99 = self.var_99 * 100.0,
            cvar_99 = self.cvar_99 * 100.0,
            roll_mean = self.rolling_sharpe_mean_63d,
            roll_min = self.rolling_sharpe_min_63d,
            roll_max = self.rolling_sharpe_max_63d,
            roll_std = self.rolling_sharpe_std_63d,
            roll_pos = self.rolling_sharpe_positive_pct_63d * 100.0,
            turnover = self.avg_daily_turnover * 100.0,
            trades = self.total_trades,
        )
    }

    /// Pretty JSON export for machine-readable comparison across runs.
    pub fn to_pretty_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Write a JSON metrics snapshot to disk.
    pub fn write_json_file(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let json = self
            .to_pretty_json()
            .map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Compare scalar metrics against a baseline result.
    pub fn compare_against(&self, baseline: &Self) -> Vec<MetricComparisonRow> {
        let mut rows = Vec::new();

        push_metric(
            &mut rows,
            "sharpe_ratio",
            baseline.sharpe_ratio,
            self.sharpe_ratio,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "annualized_return",
            baseline.annualized_return,
            self.annualized_return,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "annualized_vol",
            baseline.annualized_vol,
            self.annualized_vol,
            DirectionPreference::LowerIsBetter,
        );
        push_metric(
            &mut rows,
            "max_drawdown",
            baseline.max_drawdown,
            self.max_drawdown,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "max_drawdown_duration_days",
            baseline.max_drawdown_duration_days as f64,
            self.max_drawdown_duration_days as f64,
            DirectionPreference::LowerIsBetter,
        );
        push_metric(
            &mut rows,
            "calmar_ratio",
            baseline.calmar_ratio,
            self.calmar_ratio,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "sortino_ratio",
            baseline.sortino_ratio,
            self.sortino_ratio,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "var_95",
            baseline.var_95,
            self.var_95,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "cvar_95",
            baseline.cvar_95,
            self.cvar_95,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "var_99",
            baseline.var_99,
            self.var_99,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "cvar_99",
            baseline.cvar_99,
            self.cvar_99,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "rolling_sharpe_mean_63d",
            baseline.rolling_sharpe_mean_63d,
            self.rolling_sharpe_mean_63d,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "rolling_sharpe_min_63d",
            baseline.rolling_sharpe_min_63d,
            self.rolling_sharpe_min_63d,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "rolling_sharpe_max_63d",
            baseline.rolling_sharpe_max_63d,
            self.rolling_sharpe_max_63d,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "rolling_sharpe_std_63d",
            baseline.rolling_sharpe_std_63d,
            self.rolling_sharpe_std_63d,
            DirectionPreference::LowerIsBetter,
        );
        push_metric(
            &mut rows,
            "rolling_sharpe_positive_pct_63d",
            baseline.rolling_sharpe_positive_pct_63d,
            self.rolling_sharpe_positive_pct_63d,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "avg_daily_turnover",
            baseline.avg_daily_turnover,
            self.avg_daily_turnover,
            DirectionPreference::LowerIsBetter,
        );
        push_metric(
            &mut rows,
            "total_return",
            baseline.total_return,
            self.total_return,
            DirectionPreference::HigherIsBetter,
        );
        push_metric(
            &mut rows,
            "total_trades",
            baseline.total_trades as f64,
            self.total_trades as f64,
            DirectionPreference::Neutral,
        );

        rows
    }
}

#[derive(Debug, Clone, Copy)]
enum DirectionPreference {
    HigherIsBetter,
    LowerIsBetter,
    Neutral,
}

impl DirectionPreference {
    fn as_str(self) -> &'static str {
        match self {
            Self::HigherIsBetter => "higher",
            Self::LowerIsBetter => "lower",
            Self::Neutral => "neutral",
        }
    }

    fn assess(self, delta: f64) -> &'static str {
        if delta.abs() < 1e-12 {
            return "unchanged";
        }

        match self {
            Self::HigherIsBetter => {
                if delta > 0.0 {
                    "better"
                } else {
                    "worse"
                }
            }
            Self::LowerIsBetter => {
                if delta < 0.0 {
                    "better"
                } else {
                    "worse"
                }
            }
            Self::Neutral => "changed",
        }
    }
}

fn push_metric(
    rows: &mut Vec<MetricComparisonRow>,
    metric: &str,
    baseline: f64,
    candidate: f64,
    preference: DirectionPreference,
) {
    let delta = candidate - baseline;
    let scale = baseline.abs().max(1e-12);
    let delta_pct = if baseline.abs() < 1e-12 && candidate.abs() < 1e-12 {
        0.0
    } else {
        delta / scale
    };

    rows.push(MetricComparisonRow {
        metric: metric.to_string(),
        baseline,
        candidate,
        delta,
        delta_pct,
        preferred_direction: preference.as_str().to_string(),
        assessment: preference.assess(delta).to_string(),
    });
}

fn historical_var_cvar(returns: &[f64], confidence: f64) -> (f64, f64) {
    if returns.is_empty() {
        return (0.0, 0.0);
    }

    let alpha = (1.0 - confidence).clamp(0.0, 1.0);
    let mut sorted = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let tail_count = ((sorted.len() as f64) * alpha).floor().max(1.0) as usize;
    let tail = &sorted[..tail_count.min(sorted.len())];
    let var = *tail.last().unwrap_or(&0.0);
    let cvar = tail.iter().sum::<f64>() / tail.len() as f64;
    (var, cvar)
}

fn rolling_sharpes(returns: &[f64], window: usize) -> Vec<f64> {
    if window < 2 {
        return Vec::new();
    }

    returns
        .windows(window)
        .map(|w| {
            let mean_daily = mean(w);
            let sd_daily = std_dev(w);
            if sd_daily > 1e-8 {
                mean_daily / sd_daily * TRADING_DAYS_PER_YEAR.sqrt()
            } else {
                0.0
            }
        })
        .collect()
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn average_daily_turnover(snapshots: &[Snapshot]) -> f64 {
    if snapshots.len() < 2 {
        return 0.0;
    }

    let mut turnover_sum = 0.0;
    let mut days = 0usize;

    for pair in snapshots.windows(2) {
        let prev = &pair[0];
        let curr = &pair[1];

        if curr.nav.abs() < 1e-8 {
            continue;
        }

        let mut delta = 0.0;

        for (symbol, curr_notional) in &curr.position_notionals {
            let prev_notional = prev.position_notionals.get(symbol).copied().unwrap_or(0.0);
            delta += (curr_notional - prev_notional).abs();
        }

        for (symbol, prev_notional) in &prev.position_notionals {
            if !curr.position_notionals.contains_key(symbol) {
                delta += prev_notional.abs();
            }
        }

        turnover_sum += delta / curr.nav.abs();
        days += 1;
    }

    if days == 0 {
        0.0
    } else {
        turnover_sum / days as f64
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
                position_notionals: HashMap::new(),
                signals: HashMap::new(),
                fills: Vec::new(),
            })
            .collect()
    }

    fn make_snapshots_with_notionals(
        navs: &[f64],
        notionals: &[Vec<(&str, f64)>],
    ) -> Vec<Snapshot> {
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
                position_notionals: notionals
                    .get(i)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(sym, val)| (sym.to_string(), val))
                    .collect(),
                signals: HashMap::new(),
                fills: Vec::new(),
            })
            .collect()
    }

    fn make_navs_from_returns(initial_nav: f64, returns: &[f64]) -> Vec<f64> {
        let mut navs = Vec::with_capacity(returns.len() + 1);
        let mut nav = initial_nav;
        navs.push(nav);
        for &ret in returns {
            nav *= 1.0 + ret;
            navs.push(nav);
        }
        navs
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
        assert!(text.contains("Daily VaR 95%"));
        assert!(text.contains("Roll Sharpe 63d"));
        assert!(text.contains("Avg Daily Turn"));
        assert!(text.contains("Total Trades"));
    }

    #[test]
    fn pretty_json_contains_new_metrics() {
        let navs: Vec<f64> = (0..100).map(|i| 1_000_000.0 * 1.001_f64.powi(i)).collect();
        let snaps = make_snapshots(&navs);
        let result = BacktestResult::from_snapshots(&snaps).unwrap();
        let json = result.to_pretty_json().unwrap();

        assert!(json.contains("\"var_95\""));
        assert!(json.contains("\"rolling_sharpe_mean_63d\""));
        assert!(json.contains("\"avg_daily_turnover\""));
    }

    #[test]
    fn compare_against_includes_expected_rows() {
        let baseline = BacktestResult::from_snapshots(&make_snapshots(&[100.0, 101.0, 102.0])).unwrap();
        let candidate = BacktestResult::from_snapshots(&make_snapshots(&[100.0, 102.0, 104.0])).unwrap();

        let rows = candidate.compare_against(&baseline);
        assert!(rows.iter().any(|r| r.metric == "sharpe_ratio"));
        assert!(rows.iter().any(|r| r.metric == "var_95"));
        assert!(rows.iter().any(|r| r.metric == "avg_daily_turnover"));
        assert!(rows.iter().any(|r| r.metric == "total_trades"));
        assert!(rows.iter().any(|r| r.metric == "sharpe_ratio" && r.assessment == "better"));
    }

    #[test]
    fn std_dev_correct() {
        let vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = super::std_dev(&vals);
        // Sample std dev: sqrt(32/7) ≈ 2.138
        assert!((sd - 2.138).abs() < 0.01);
    }

    #[test]
    fn historical_var_cvar_computed_correctly() {
        let mut returns = vec![0.01; 95];
        returns.extend([-0.05, -0.04, -0.03, -0.02, -0.01]);

        let navs = make_navs_from_returns(1_000_000.0, &returns);
        let snaps = make_snapshots(&navs);
        let result = BacktestResult::from_snapshots(&snaps).unwrap();

        assert!((result.var_95 - (-0.01)).abs() < 1e-9);
        assert!((result.cvar_95 - (-0.03)).abs() < 1e-9);
        assert!((result.var_99 - (-0.05)).abs() < 1e-9);
        assert!((result.cvar_99 - (-0.05)).abs() < 1e-9);
    }

    #[test]
    fn rolling_sharpe_metrics_capture_regime_shift() {
        let mut returns = Vec::new();
        for i in 0..63 {
            returns.push(if i % 2 == 0 { 0.004 } else { 0.002 });
        }
        for i in 0..63 {
            returns.push(if i % 2 == 0 { -0.004 } else { -0.002 });
        }

        let navs = make_navs_from_returns(1_000_000.0, &returns);
        let snaps = make_snapshots(&navs);
        let result = BacktestResult::from_snapshots(&snaps).unwrap();

        assert!(result.rolling_sharpe_max_63d > 0.0);
        assert!(result.rolling_sharpe_min_63d < 0.0);
        assert!(result.rolling_sharpe_std_63d > 0.0);
        assert!(result.rolling_sharpe_positive_pct_63d > 0.0);
        assert!(result.rolling_sharpe_positive_pct_63d < 1.0);
    }

    #[test]
    fn rolling_sharpe_metrics_default_to_zero_when_window_unavailable() {
        let returns = [0.01, -0.01, 0.02, -0.02, 0.01];
        let navs = make_navs_from_returns(1_000_000.0, &returns);
        let snaps = make_snapshots(&navs);
        let result = BacktestResult::from_snapshots(&snaps).unwrap();

        assert_eq!(result.rolling_sharpe_mean_63d, 0.0);
        assert_eq!(result.rolling_sharpe_min_63d, 0.0);
        assert_eq!(result.rolling_sharpe_max_63d, 0.0);
        assert_eq!(result.rolling_sharpe_std_63d, 0.0);
        assert_eq!(result.rolling_sharpe_positive_pct_63d, 0.0);
    }

    #[test]
    fn average_daily_turnover_computed_correctly() {
        let snaps = make_snapshots_with_notionals(
            &[100.0, 100.0, 100.0],
            &[
                vec![("A", 10.0)],
                vec![("A", 15.0), ("B", -5.0)],
                vec![("B", -10.0)],
            ],
        );

        let result = BacktestResult::from_snapshots(&snaps).unwrap();

        // Day 1->2: |15-10| + |-5-0| = 10 => 10/100 = 0.10
        // Day 2->3: |0-15| + |-10-(-5)| = 20 => 20/100 = 0.20
        // Average = 0.15
        assert!((result.avg_daily_turnover - 0.15).abs() < 1e-12);
    }
}
