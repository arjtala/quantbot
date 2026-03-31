use std::collections::{BTreeSet, HashMap};

use chrono::NaiveDate;

use crate::agents::tsmom::TSMOMAgent;
use crate::core::bar::{Bar, BarSeries};
use crate::core::portfolio::{Fill, Order, OrderSide, PortfolioState, Position};
use crate::core::signal::{Signal, SignalDirection};

/// Backtest configuration.
pub struct BacktestConfig {
    pub initial_cash: f64,
    pub slippage_bps: f64,
    pub vol_target: f64,
    pub max_gross_leverage: f64,
    pub max_position_pct: f64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_cash: 1_000_000.0,
            slippage_bps: 5.0,
            vol_target: 0.40,
            max_gross_leverage: 2.0,
            max_position_pct: 0.20,
        }
    }
}

/// Daily snapshot of backtest state.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub date: NaiveDate,
    pub nav: f64,
    pub cash: f64,
    pub gross_exposure: f64,
    pub net_exposure: f64,
    pub positions: HashMap<String, f64>,
    /// Per-instrument notional exposure (quantity * close price).
    pub position_notionals: HashMap<String, f64>,
    pub signals: HashMap<String, Signal>,
    pub fills: Vec<Fill>,
}

/// Event-driven backtest engine with next-open execution.
///
/// At each bar:
/// 1. Execute pending orders at today's open (next-open execution)
/// 2. Mark-to-market existing positions at today's close
/// 3. Generate signals using data up to today's close
/// 4. Convert signals to target quantities for tomorrow's open
/// 5. Record snapshot (only if date >= eval_start)
pub struct BacktestEngine {
    config: BacktestConfig,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(BacktestConfig::default())
    }

    /// Run backtest across multiple instruments.
    ///
    /// The engine processes all bars from min_history onwards, computing signals
    /// and executing trades throughout. However, only snapshots on or after
    /// `eval_start` are included in the returned results. This separates the
    /// warm-up period (needed for lookback computation) from the evaluation
    /// period (used for Sharpe calculation).
    ///
    /// Pass `None` for `eval_start` to include all snapshots (no warm-up separation).
    pub fn run(
        &self,
        agent: &TSMOMAgent,
        bars_by_instrument: &HashMap<String, BarSeries>,
        min_history: usize,
        eval_start: Option<NaiveDate>,
    ) -> Vec<Snapshot> {
        let all_dates = self.get_all_dates(bars_by_instrument);

        if all_dates.len() <= min_history {
            return Vec::new();
        }

        // Build per-instrument date->index lookup for O(1) bar access
        let bar_indexes: HashMap<&str, HashMap<NaiveDate, usize>> = bars_by_instrument
            .iter()
            .map(|(sym, series)| {
                let idx: HashMap<NaiveDate, usize> = series
                    .bars()
                    .iter()
                    .enumerate()
                    .map(|(i, b)| (b.date, i))
                    .collect();
                (sym.as_str(), idx)
            })
            .collect();

        let dates: Vec<NaiveDate> = all_dates.into_iter().collect();
        let mut portfolio = PortfolioState::new(self.config.initial_cash);
        let mut snapshots = Vec::new();
        let mut pending_targets: HashMap<String, f64> = HashMap::new();

        for &today in &dates[min_history..] {
            // Collect today's prices
            let mut close_prices: HashMap<String, f64> = HashMap::new();
            let mut open_prices: HashMap<String, f64> = HashMap::new();

            for (sym, series) in bars_by_instrument {
                if let Some(&idx) = bar_indexes[sym.as_str()].get(&today) {
                    let bar = &series.bars()[idx];
                    close_prices.insert(sym.clone(), bar.close);
                    open_prices.insert(sym.clone(), bar.open);
                }
            }

            // Step 1: Execute pending orders at today's open
            let fills = if pending_targets.is_empty() {
                Vec::new()
            } else {
                self.rebalance(&mut portfolio, &mut pending_targets, &open_prices)
            };

            // Step 2: Mark positions to today's close
            Self::mark_to_market(&mut portfolio, &close_prices);

            // Step 3: Generate signals using data up to today
            let mut signals: HashMap<String, Signal> = HashMap::new();
            let mut target_weights: HashMap<String, f64> = HashMap::new();

            for (sym, series) in bars_by_instrument {
                if let Some(&today_idx) = bar_indexes[sym.as_str()].get(&today) {
                    let history_len = today_idx + 1;
                    if history_len < min_history {
                        continue;
                    }

                    let history_bars: Vec<Bar> = series.bars()[..=today_idx].to_vec();
                    if let Ok(history) = BarSeries::new(history_bars) {
                        let sig = agent.generate_signal(&history, sym);
                        let weight = if sig.direction != SignalDirection::Flat {
                            TSMOMAgent::compute_target_weight(&sig)
                        } else {
                            0.0
                        };
                        target_weights.insert(sym.clone(), weight);
                        signals.insert(sym.clone(), sig);
                    }
                }
            }

            // Apply risk limits
            self.apply_risk_limits(&mut target_weights);

            // Convert weights to target quantities
            let nav = portfolio.nav();
            pending_targets.clear();
            for (sym, weight) in &target_weights {
                if let Some(&px) = close_prices.get(sym) {
                    if px > 0.0 {
                        let target_notional = weight * nav;
                        pending_targets.insert(sym.clone(), target_notional / px);
                    }
                }
            }

            // Step 4: Record snapshot (only during eval period)
            let in_eval = eval_start.is_none_or(|es| today >= es);
            if in_eval {
                let position_notionals: HashMap<String, f64> = portfolio
                    .positions
                    .iter()
                    .map(|(s, p)| {
                        let px = close_prices.get(s).copied().unwrap_or(p.avg_entry_price);
                        (s.clone(), p.quantity * px)
                    })
                    .collect();
                snapshots.push(Snapshot {
                    date: today,
                    nav: portfolio.nav(),
                    cash: portfolio.cash,
                    gross_exposure: portfolio.gross_exposure(Some(&close_prices)),
                    net_exposure: portfolio.net_exposure(Some(&close_prices)),
                    positions: portfolio
                        .positions
                        .iter()
                        .map(|(s, p)| (s.clone(), p.quantity))
                        .collect(),
                    position_notionals,
                    signals,
                    fills,
                });
            }
        }

        snapshots
    }

    /// Get the sorted union of all dates across instruments.
    fn get_all_dates(&self, bars_by_instrument: &HashMap<String, BarSeries>) -> BTreeSet<NaiveDate> {
        let mut dates = BTreeSet::new();
        for series in bars_by_instrument.values() {
            for bar in series.bars() {
                dates.insert(bar.date);
            }
        }
        dates
    }

    /// Rebalance portfolio to target quantities at open prices.
    fn rebalance(
        &self,
        portfolio: &mut PortfolioState,
        target_quantities: &mut HashMap<String, f64>,
        open_prices: &HashMap<String, f64>,
    ) -> Vec<Fill> {
        let mut fills = Vec::new();

        // Close positions for instruments no longer targeted
        for sym in portfolio.positions.keys().cloned().collect::<Vec<_>>() {
            target_quantities.entry(sym).or_insert(0.0);
        }

        for (sym, &target_qty) in target_quantities.iter() {
            let Some(&price) = open_prices.get(sym) else {
                continue;
            };

            let current_qty = portfolio
                .positions
                .get(sym)
                .map(|p| p.quantity)
                .unwrap_or(0.0);

            let delta = target_qty - current_qty;
            if delta.abs() < 1e-8 {
                continue;
            }

            let slippage = self.config.slippage_bps / 10_000.0;
            let (fill_price, side) = if delta > 0.0 {
                (price * (1.0 + slippage), OrderSide::Buy)
            } else {
                (price * (1.0 - slippage), OrderSide::Sell)
            };

            let order = Order::new(sym.clone(), side, delta.abs());
            fills.push(Fill {
                order,
                fill_price,
                fill_quantity: delta.abs(),
                timestamp: chrono::Utc::now(),
                slippage_bps: self.config.slippage_bps,
            });

            // Update portfolio
            let cost = delta * fill_price;
            portfolio.cash -= cost;

            if target_qty.abs() < 1e-8 {
                portfolio.positions.remove(sym);
            } else {
                portfolio.positions.insert(
                    sym.clone(),
                    Position {
                        instrument: sym.clone(),
                        quantity: target_qty,
                        avg_entry_price: fill_price,
                        point_value: 1.0,
                    },
                );
            }
        }

        fills
    }

    /// Mark positions to current market prices.
    fn mark_to_market(portfolio: &mut PortfolioState, prices: &HashMap<String, f64>) {
        for (sym, pos) in portfolio.positions.iter_mut() {
            if let Some(&px) = prices.get(sym) {
                pos.avg_entry_price = px;
            }
        }
    }

    /// Apply risk limits: scale gross leverage first, then cap individual positions.
    /// Order matches the Python reference implementation.
    fn apply_risk_limits(&self, weights: &mut HashMap<String, f64>) {
        // Scale down if gross leverage exceeds limit
        let gross: f64 = weights.values().map(|w| w.abs()).sum();
        if gross > self.config.max_gross_leverage {
            let scale = self.config.max_gross_leverage / gross;
            for w in weights.values_mut() {
                *w *= scale;
            }
        }

        // Then cap individual position weights
        let cap = self.config.max_position_pct * self.config.max_gross_leverage;
        for w in weights.values_mut() {
            if w.abs() > cap {
                *w = w.signum() * cap;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bar::Bar;

    /// Generate daily bars with a steady trend for testing.
    fn trending_bars(n: usize, start_price: f64, daily_ret: f64) -> BarSeries {
        let mut bars = Vec::with_capacity(n);
        let mut price = start_price;
        let base = NaiveDate::from_ymd_opt(2023, 1, 2).unwrap();
        for i in 0..n {
            let open = price;
            price *= 1.0 + daily_ret;
            bars.push(Bar {
                date: base + chrono::Days::new(i as u64),
                open,
                high: open.max(price) * 1.005,
                low: open.min(price) * 0.995,
                close: price,
                volume: 10000.0,
            });
        }
        BarSeries::new(bars).unwrap()
    }

    #[test]
    fn engine_produces_snapshots() {
        let bars = trending_bars(300, 100.0, 0.001);
        let mut instruments = HashMap::new();
        instruments.insert("TEST".into(), bars);

        let engine = BacktestEngine::with_defaults();
        let agent = TSMOMAgent::new();
        let snaps = engine.run(&agent, &instruments, 253, None);
        assert!(!snaps.is_empty());
        assert_eq!(snaps.len(), 300 - 253);
    }

    #[test]
    fn eval_start_filters_warmup() {
        let bars = trending_bars(300, 100.0, 0.001);
        let mut instruments = HashMap::new();
        instruments.insert("TEST".into(), bars);

        let engine = BacktestEngine::with_defaults();
        let agent = TSMOMAgent::new();

        // Without eval_start: all snapshots from min_history onwards
        let all = engine.run(&agent, &instruments, 253, None);

        // With eval_start at day 280: only the last ~20 snapshots
        let eval_start = NaiveDate::from_ymd_opt(2023, 1, 2).unwrap() + chrono::Days::new(280);
        let filtered = engine.run(&agent, &instruments, 253, Some(eval_start));

        assert!(filtered.len() < all.len());
        assert!(filtered.first().unwrap().date >= eval_start);
        // NAV should differ because the engine still trades during warmup
        // but the filtered snapshots only cover the eval window
    }

    #[test]
    fn nav_starts_at_initial_cash() {
        let bars = trending_bars(300, 100.0, 0.001);
        let mut instruments = HashMap::new();
        instruments.insert("TEST".into(), bars);

        let config = BacktestConfig {
            initial_cash: 500_000.0,
            ..BacktestConfig::default()
        };
        let engine = BacktestEngine::new(config);
        let agent = TSMOMAgent::new();
        let snaps = engine.run(&agent, &instruments, 253, None);

        assert!((snaps[0].nav - 500_000.0).abs() < 1.0);
    }

    #[test]
    fn uptrend_generates_positive_nav() {
        let bars = trending_bars(300, 100.0, 0.002);
        let mut instruments = HashMap::new();
        instruments.insert("TEST".into(), bars);

        let engine = BacktestEngine::with_defaults();
        let agent = TSMOMAgent::new();
        let snaps = engine.run(&agent, &instruments, 253, None);

        let first_nav = snaps.first().unwrap().nav;
        let last_nav = snaps.last().unwrap().nav;
        assert!(
            last_nav > first_nav,
            "NAV should increase in uptrend: first={first_nav}, last={last_nav}"
        );
    }

    #[test]
    fn risk_limits_cap_leverage() {
        let engine = BacktestEngine::with_defaults();
        let mut weights = HashMap::new();
        weights.insert("A".into(), 1.5);
        weights.insert("B".into(), 1.5);
        engine.apply_risk_limits(&mut weights);
        let gross: f64 = weights.values().map(|w| w.abs()).sum();
        assert!(gross <= 2.0 + 1e-10);
    }

    #[test]
    fn empty_bars_returns_empty_snapshots() {
        let instruments: HashMap<String, BarSeries> = HashMap::new();
        let engine = BacktestEngine::with_defaults();
        let agent = TSMOMAgent::new();
        let snaps = engine.run(&agent, &instruments, 253, None);
        assert!(snaps.is_empty());
    }
}
