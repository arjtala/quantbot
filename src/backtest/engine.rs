use std::collections::{BTreeSet, HashMap};

use chrono::NaiveDate;

use crate::agents::tsmom::TSMOMAgent;
use crate::core::bar::{Bar, BarSeries};
use crate::core::portfolio::{Fill, Order, OrderSide, PortfolioState, Position};
use crate::core::signal::{Signal, SignalDirection};
use crate::execution::router::{ExecutionRouter, SizedOrder, SpreadCostTracker};

/// Backtest configuration.
pub struct BacktestConfig {
    pub initial_cash: f64,
    pub vol_target: f64,
    pub max_gross_leverage: f64,
    pub max_position_pct: f64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_cash: 1_000_000.0,
            vol_target: 0.40,
            max_gross_leverage: 2.0,
            max_position_pct: 0.20,
        }
    }
}

/// Daily snapshot of backtest state.
#[derive(Debug, Clone, serde::Serialize)]
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

/// Single-shot target snapshot for paper-trade mode.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TargetSnapshot {
    pub date: NaiveDate,
    pub signals: HashMap<String, Signal>,
    pub raw_weights: HashMap<String, f64>,
    pub target_weights: HashMap<String, f64>,
    pub target_quantities: HashMap<String, f64>,
    pub orders: Vec<SizedOrder>,
    pub total_margin: f64,
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
    router: ExecutionRouter,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self {
            config,
            router: ExecutionRouter::with_ig_defaults(),
        }
    }

    pub fn new_with_router(config: BacktestConfig, router: ExecutionRouter) -> Self {
        Self { config, router }
    }

    pub fn with_defaults() -> Self {
        Self::new(BacktestConfig::default())
    }

    /// Generate target positions for a single point in time (paper-trade mode).
    ///
    /// Runs the signal pipeline on the latest available data for each instrument,
    /// applies risk limits, sizes via the execution router, and diffs against
    /// `current_quantities` to produce orders.
    pub fn generate_targets(
        &self,
        agent: &TSMOMAgent,
        bars_by_instrument: &HashMap<String, BarSeries>,
        current_quantities: &HashMap<String, f64>,
        nav: f64,
        min_history: usize,
    ) -> TargetSnapshot {
        let mut signals: HashMap<String, Signal> = HashMap::new();
        let mut raw_weights: HashMap<String, f64> = HashMap::new();
        let mut close_prices: HashMap<String, f64> = HashMap::new();
        let mut max_date = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();

        // Generate signals for each instrument with sufficient history
        for (sym, series) in bars_by_instrument {
            if series.bars().len() < min_history {
                continue;
            }

            let sig = agent.generate_signal(series, sym);
            let weight = if sig.direction != SignalDirection::Flat {
                TSMOMAgent::compute_target_weight(&sig)
            } else {
                0.0
            };

            let last_bar = series.bars().last().unwrap();
            close_prices.insert(sym.clone(), last_bar.close);
            if last_bar.date > max_date {
                max_date = last_bar.date;
            }

            raw_weights.insert(sym.clone(), weight);
            signals.insert(sym.clone(), sig);
        }

        // Apply risk limits to a clone of the weights
        let mut target_weights = raw_weights.clone();
        self.apply_risk_limits(&mut target_weights);

        // Size from weights to lot-rounded quantities
        let mut target_quantities: HashMap<String, f64> = HashMap::new();
        for (sym, &weight) in &target_weights {
            if let Some(&px) = close_prices.get(sym) {
                if px > 0.0 {
                    let qty = self.router.size_from_weight(sym, weight, nav, px);
                    target_quantities.insert(sym.clone(), qty);
                }
            }
        }

        // Diff against current positions to create orders
        let mut orders = Vec::new();
        for (sym, &target_qty) in &target_quantities {
            let current_qty = current_quantities.get(sym).copied().unwrap_or(0.0);
            if let Some(order) = self.router.create_sized_order(
                sym,
                target_qty,
                current_qty,
                close_prices[sym],
                1.0, // opening from flat or adjusting — use 1.0
            ) {
                orders.push(order);
            }
        }

        // Sort orders by instrument for deterministic output
        orders.sort_by(|a, b| a.instrument.cmp(&b.instrument));

        let total_margin = self.router.total_margin(&target_quantities, &close_prices);

        TargetSnapshot {
            date: max_date,
            signals,
            raw_weights,
            target_weights,
            target_quantities,
            orders,
            total_margin,
        }
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
        let mut spread_tracker = SpreadCostTracker::new();

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
                self.rebalance(
                    &mut portfolio,
                    &mut pending_targets,
                    &open_prices,
                    &mut spread_tracker,
                )
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

            // Convert weights to target quantities using execution router
            let nav = portfolio.nav();
            pending_targets.clear();
            for (sym, weight) in &target_weights {
                if let Some(&px) = close_prices.get(sym) {
                    if px > 0.0 {
                        let qty = self.router.size_from_weight(sym, *weight, nav, px);
                        pending_targets.insert(sym.clone(), qty);
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
                        (s.clone(), p.quantity * px * p.point_value)
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
    fn get_all_dates(
        &self,
        bars_by_instrument: &HashMap<String, BarSeries>,
    ) -> BTreeSet<NaiveDate> {
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
        spread_tracker: &mut SpreadCostTracker,
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

            let spec = self.router.get_spec(sym);
            let point_value = spec.point_value;

            // Spread cost via tracker: direction-aware multiplier
            let cost_mult = spread_tracker.cost_multiplier(sym, target_qty);
            let trade_notional = delta.abs() * price * point_value;
            let spread_cost = spec.spread_cost(trade_notional) * cost_mult;

            let side = if delta > 0.0 {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            };

            let order = Order::new(sym.clone(), side, delta.abs());
            fills.push(Fill {
                order,
                fill_price: price,
                fill_quantity: delta.abs(),
                timestamp: chrono::Utc::now(),
                slippage_bps: spec.spread_bps,
            });

            // Update portfolio: cash changes by notional + spread cost
            portfolio.cash -= delta * price * point_value;
            portfolio.cash -= spread_cost;

            if target_qty.abs() < 1e-8 {
                portfolio.positions.remove(sym);
            } else {
                portfolio.positions.insert(
                    sym.clone(),
                    Position {
                        instrument: sym.clone(),
                        quantity: target_qty,
                        avg_entry_price: price,
                        point_value,
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

    #[test]
    fn futures_sizing_uses_point_value() {
        // GC=F has point_value=100, so quantity should be much smaller
        let router = ExecutionRouter::with_ig_defaults();
        let qty = router.size_from_weight("GC=F", 0.20, 1_000_000.0, 2000.0);
        // target_notional = 0.20 * 1_000_000 = 200_000
        // raw_qty = 200_000 / (2000 * 100) = 1.0
        assert_eq!(qty, 1.0);

        // Compare with equity: same weight/nav/price but pv=1
        let eq_qty = router.size_from_weight("SPY", 0.20, 1_000_000.0, 500.0);
        // raw_qty = 200_000 / (500 * 1) = 400
        assert_eq!(eq_qty, 400.0);
    }

    #[test]
    fn spread_cost_applied_in_rebalance() {
        // Build a minimal engine + manually call rebalance to verify spread costs
        let mut specs = std::collections::HashMap::new();
        specs.insert(
            "X".to_string(),
            crate::execution::router::ContractSpec {
                symbol: "X".to_string(),
                asset_class: crate::execution::router::AssetClass::Equity,
                point_value: 1.0,
                min_deal_size: 1.0,
                lot_step: 1.0,
                margin_pct: 0.20,
                spread_bps: 100.0, // 1% spread for easy math
            },
        );
        let router = ExecutionRouter::new(specs);
        let config = BacktestConfig {
            initial_cash: 100_000.0,
            ..BacktestConfig::default()
        };
        let engine = BacktestEngine::new_with_router(config, router);

        let mut portfolio = PortfolioState::new(100_000.0);
        let open_prices: HashMap<String, f64> = [("X".to_string(), 100.0)].into();
        let mut tracker = SpreadCostTracker::new();

        // Buy 100 shares at $100 → notional = 10_000, spread = 1% * 10_000 = 100
        // Open from flat → multiplier = 1.0
        let mut targets: HashMap<String, f64> = [("X".to_string(), 100.0)].into();
        engine.rebalance(&mut portfolio, &mut targets, &open_prices, &mut tracker);

        // Cash should be 100_000 - 10_000 (notional) - 100 (spread) = 89_900
        assert!(
            (portfolio.cash - 89_900.0).abs() < 1e-6,
            "Expected 89_900, got {}",
            portfolio.cash
        );

        // Hold same position → multiplier = 0.0, no spread cost
        let mut targets2: HashMap<String, f64> = [("X".to_string(), 100.0)].into();
        let cash_before = portfolio.cash;
        engine.rebalance(&mut portfolio, &mut targets2, &open_prices, &mut tracker);
        assert!(
            (portfolio.cash - cash_before).abs() < 1e-6,
            "Holding should not incur spread cost"
        );

        // Flip to short → multiplier = 2.0
        let mut targets3: HashMap<String, f64> = [("X".to_string(), -100.0)].into();
        engine.rebalance(&mut portfolio, &mut targets3, &open_prices, &mut tracker);
        // delta = -200, notional_cost = -(-200) * 100 * 1 = +20_000 cash back
        // trade_notional = 200 * 100 * 1 = 20_000, spread = 1% * 20_000 * 2 = 400
        let expected_cash = cash_before + 20_000.0 - 400.0;
        assert!(
            (portfolio.cash - expected_cash).abs() < 1e-6,
            "Expected {expected_cash}, got {}",
            portfolio.cash
        );
    }

    // ── generate_targets tests ──────────────────────────────────

    #[test]
    fn generate_targets_produces_signals_and_orders() {
        let bars = trending_bars(300, 100.0, 0.002);
        let mut instruments = HashMap::new();
        instruments.insert("TEST".into(), bars);

        let engine = BacktestEngine::with_defaults();
        let agent = TSMOMAgent::new();
        let snapshot = engine.generate_targets(
            &agent,
            &instruments,
            &HashMap::new(), // flat positions
            1_000_000.0,
            253,
        );

        // Should produce a Long signal for an uptrend
        assert!(!snapshot.signals.is_empty());
        let sig = snapshot.signals.get("TEST").unwrap();
        assert_eq!(sig.direction, SignalDirection::Long);

        // Should produce orders since we're starting flat
        assert!(!snapshot.orders.is_empty());
        assert!(snapshot.total_margin > 0.0);
    }

    #[test]
    fn generate_targets_flat_when_insufficient_data() {
        let bars = trending_bars(100, 100.0, 0.002);
        let mut instruments = HashMap::new();
        instruments.insert("TEST".into(), bars);

        let engine = BacktestEngine::with_defaults();
        let agent = TSMOMAgent::new();
        let snapshot = engine.generate_targets(
            &agent,
            &instruments,
            &HashMap::new(),
            1_000_000.0,
            253, // need 253 bars but only have 100
        );

        assert!(snapshot.signals.is_empty());
        assert!(snapshot.orders.is_empty());
        assert_eq!(snapshot.total_margin, 0.0);
    }

    #[test]
    fn generate_targets_no_orders_when_already_at_target() {
        let bars = trending_bars(300, 100.0, 0.002);
        let mut instruments = HashMap::new();
        instruments.insert("TEST".into(), bars);

        let engine = BacktestEngine::with_defaults();
        let agent = TSMOMAgent::new();

        // First run: get target quantities from flat
        let snap1 =
            engine.generate_targets(&agent, &instruments, &HashMap::new(), 1_000_000.0, 253);

        // Second run: already at target quantities
        let snap2 = engine.generate_targets(
            &agent,
            &instruments,
            &snap1.target_quantities,
            1_000_000.0,
            253,
        );

        // No orders needed — already at target
        assert!(snap2.orders.is_empty());
    }

    #[test]
    fn generate_targets_risk_limits_applied() {
        // Create 6 instruments all with strong trends → gross weight should be capped at 2.0
        let mut instruments = HashMap::new();
        for i in 0..6 {
            let sym = format!("INST{}", i);
            instruments.insert(sym, trending_bars(300, 100.0, 0.003));
        }

        let engine = BacktestEngine::with_defaults();
        let agent = TSMOMAgent::new();
        let snapshot =
            engine.generate_targets(&agent, &instruments, &HashMap::new(), 1_000_000.0, 253);

        // Raw weights should sum to more than 2.0 (6 strong trends)
        let raw_gross: f64 = snapshot.raw_weights.values().map(|w| w.abs()).sum();
        assert!(raw_gross > 2.0, "Expected raw gross > 2.0, got {raw_gross}");

        // Target weights should be capped at max_gross_leverage = 2.0
        let target_gross: f64 = snapshot.target_weights.values().map(|w| w.abs()).sum();
        assert!(
            target_gross <= 2.0 + 1e-10,
            "Expected target gross <= 2.0, got {target_gross}"
        );
    }
}
