use std::collections::HashMap;

use crate::core::portfolio::OrderSide;

// ─── Contract Specification ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AssetClass {
    Equity,
    Futures,
    Fx,
}

#[derive(Debug, Clone)]
pub struct ContractSpec {
    pub symbol: String,
    pub asset_class: AssetClass,
    pub point_value: f64,
    pub min_deal_size: f64,
    pub lot_step: f64,
    pub margin_pct: f64,
    pub spread_bps: f64,
}

impl ContractSpec {
    /// Round raw quantity to the nearest lot_step, preserving sign.
    /// Returns 0.0 if the absolute result is below min_deal_size.
    pub fn round_lots(&self, raw_qty: f64) -> f64 {
        if raw_qty == 0.0 {
            return 0.0;
        }
        let sign = raw_qty.signum();
        let abs_qty = raw_qty.abs();
        let rounded = (abs_qty / self.lot_step).round() * self.lot_step;
        if rounded < self.min_deal_size {
            return 0.0;
        }
        sign * rounded
    }

    pub fn margin_required(&self, notional: f64) -> f64 {
        notional.abs() * self.margin_pct
    }

    pub fn spread_cost(&self, notional: f64) -> f64 {
        notional.abs() * self.spread_bps / 10_000.0
    }

    /// Fallback spec for unknown equity instruments.
    pub fn default_equity(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            asset_class: AssetClass::Equity,
            point_value: 1.0,
            min_deal_size: 1.0,
            lot_step: 1.0,
            margin_pct: 0.20,
            spread_bps: 5.0,
        }
    }

    /// IG default specs for the 6 tradeable instruments.
    pub fn ig_defaults() -> HashMap<String, ContractSpec> {
        let mut m = HashMap::new();
        m.insert("GLD".into(), ContractSpec {
            symbol: "GLD".into(),
            asset_class: AssetClass::Equity,
            point_value: 1.0,
            min_deal_size: 1.0,
            lot_step: 1.0,
            margin_pct: 0.20,
            spread_bps: 10.0,
        });
        m.insert("GC=F".into(), ContractSpec {
            symbol: "GC=F".into(),
            asset_class: AssetClass::Futures,
            point_value: 100.0,
            min_deal_size: 1.0,
            lot_step: 1.0,
            margin_pct: 0.05,
            spread_bps: 10.0,
        });
        m.insert("SPY".into(), ContractSpec {
            symbol: "SPY".into(),
            asset_class: AssetClass::Equity,
            point_value: 1.0,
            min_deal_size: 1.0,
            lot_step: 1.0,
            margin_pct: 0.20,
            spread_bps: 5.0,
        });
        for sym in &["GBPUSD=X", "USDCHF=X", "USDJPY=X"] {
            m.insert(sym.to_string(), ContractSpec {
                symbol: sym.to_string(),
                asset_class: AssetClass::Fx,
                point_value: 1.0,
                min_deal_size: 0.5,
                lot_step: 0.1,
                margin_pct: 0.0333,
                spread_bps: 3.0,
            });
        }
        m
    }
}

// ─── Sized Order ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SizedOrder {
    pub instrument: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub reference_price: f64,
    pub point_value: f64,
    pub notional: f64,
    pub margin_required: f64,
    pub spread_cost: f64,
}

// ─── Spread Cost Tracker ───────────────────────────────────────────

/// Tracks position direction per instrument to determine spread cost multipliers.
/// Mirrors Python simulate_combiner.py lines 196-212.
pub struct SpreadCostTracker {
    directions: HashMap<String, f64>,
}

impl SpreadCostTracker {
    pub fn new() -> Self {
        Self {
            directions: HashMap::new(),
        }
    }

    /// Returns the cost multiplier based on direction change:
    /// - 0.0: holding same direction (no trade)
    /// - 1.0: opening or closing a position
    /// - 2.0: flipping direction (close + open)
    pub fn cost_multiplier(&mut self, symbol: &str, new_direction: f64) -> f64 {
        let old = self.directions.get(symbol).copied().unwrap_or(0.0);
        let old_sign = sign(old);
        let new_sign = sign(new_direction);

        self.directions.insert(symbol.to_string(), new_direction);

        if old_sign == new_sign {
            // Same direction (including both flat) → no trade
            0.0
        } else if old_sign == 0 || new_sign == 0 {
            // One side is flat → open or close
            1.0
        } else {
            // Opposite signs → flip
            2.0
        }
    }
}

impl Default for SpreadCostTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn sign(x: f64) -> i8 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

// ─── Execution Router ──────────────────────────────────────────────

pub struct ExecutionRouter {
    specs: HashMap<String, ContractSpec>,
}

impl ExecutionRouter {
    pub fn new(specs: HashMap<String, ContractSpec>) -> Self {
        Self { specs }
    }

    pub fn with_ig_defaults() -> Self {
        Self::new(ContractSpec::ig_defaults())
    }

    pub fn get_spec(&self, symbol: &str) -> ContractSpec {
        self.specs
            .get(symbol)
            .cloned()
            .unwrap_or_else(|| ContractSpec::default_equity(symbol))
    }

    pub fn point_value(&self, symbol: &str) -> f64 {
        self.get_spec(symbol).point_value
    }

    /// Convert a portfolio weight to a lot-rounded quantity.
    /// `raw_qty = (weight * nav) / (price * point_value)`, then lot-rounded.
    pub fn size_from_weight(&self, symbol: &str, weight: f64, nav: f64, price: f64) -> f64 {
        if price == 0.0 {
            return 0.0;
        }
        let spec = self.get_spec(symbol);
        let target_notional = weight * nav;
        let raw_qty = target_notional / (price * spec.point_value);
        spec.round_lots(raw_qty)
    }

    /// Create a fully-specified order for a position change.
    /// Returns None if the net trade quantity rounds to zero.
    pub fn create_sized_order(
        &self,
        symbol: &str,
        target_qty: f64,
        current_qty: f64,
        price: f64,
        cost_multiplier: f64,
    ) -> Option<SizedOrder> {
        let delta = target_qty - current_qty;
        let spec = self.get_spec(symbol);
        let rounded = spec.round_lots(delta);
        if rounded == 0.0 {
            return None;
        }

        let side = if rounded > 0.0 {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };
        let notional = rounded.abs() * price * spec.point_value;

        Some(SizedOrder {
            instrument: symbol.to_string(),
            side,
            quantity: rounded.abs(),
            reference_price: price,
            point_value: spec.point_value,
            notional,
            margin_required: spec.margin_required(notional),
            spread_cost: spec.spread_cost(notional) * cost_multiplier,
        })
    }

    /// Total margin required for a set of target positions.
    pub fn total_margin(
        &self,
        targets: &HashMap<String, f64>,
        prices: &HashMap<String, f64>,
    ) -> f64 {
        targets
            .iter()
            .map(|(sym, &qty)| {
                let spec = self.get_spec(sym);
                let price = prices.get(sym).copied().unwrap_or(0.0);
                let notional = qty.abs() * price * spec.point_value;
                spec.margin_required(notional)
            })
            .sum()
    }
}

// ─── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Lot rounding ───────────────────────────────────────────

    #[test]
    fn round_lots_equity() {
        let spec = ContractSpec::ig_defaults().remove("SPY").unwrap();
        assert_eq!(spec.round_lots(3.7), 4.0);
        assert_eq!(spec.round_lots(3.4), 3.0);
    }

    #[test]
    fn round_lots_fx() {
        let spec = ContractSpec::ig_defaults().remove("GBPUSD=X").unwrap();
        // lot_step=0.1, min_deal=0.5
        assert!((spec.round_lots(1.37) - 1.4).abs() < 1e-10);
        assert!((spec.round_lots(0.54) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn round_lots_futures() {
        let spec = ContractSpec::ig_defaults().remove("GC=F").unwrap();
        assert_eq!(spec.round_lots(2.6), 3.0);
        assert_eq!(spec.round_lots(2.4), 2.0);
    }

    #[test]
    fn round_lots_preserves_sign() {
        let spec = ContractSpec::ig_defaults().remove("SPY").unwrap();
        assert_eq!(spec.round_lots(-3.7), -4.0);
        assert_eq!(spec.round_lots(-3.4), -3.0);
    }

    #[test]
    fn round_lots_zero() {
        let spec = ContractSpec::ig_defaults().remove("SPY").unwrap();
        assert_eq!(spec.round_lots(0.0), 0.0);
    }

    #[test]
    fn round_lots_sub_minimum() {
        let spec = ContractSpec::ig_defaults().remove("GBPUSD=X").unwrap();
        // 0.3 rounds to 0.3, which is < min_deal_size 0.5
        assert_eq!(spec.round_lots(0.3), 0.0);
    }

    // ── Margin ─────────────────────────────────────────────────

    #[test]
    fn margin_equity() {
        let spec = ContractSpec::ig_defaults().remove("SPY").unwrap();
        assert!((spec.margin_required(10_000.0) - 2_000.0).abs() < 1e-10);
    }

    #[test]
    fn margin_futures() {
        let spec = ContractSpec::ig_defaults().remove("GC=F").unwrap();
        assert!((spec.margin_required(200_000.0) - 10_000.0).abs() < 1e-10);
    }

    #[test]
    fn margin_fx() {
        let spec = ContractSpec::ig_defaults().remove("GBPUSD=X").unwrap();
        let expected = 10_000.0 * 0.0333;
        assert!((spec.margin_required(10_000.0) - expected).abs() < 1e-10);
    }

    #[test]
    fn margin_negative_notional() {
        let spec = ContractSpec::ig_defaults().remove("SPY").unwrap();
        // Should use absolute value
        assert!((spec.margin_required(-10_000.0) - 2_000.0).abs() < 1e-10);
    }

    // ── Spread cost ────────────────────────────────────────────

    #[test]
    fn spread_cost_forex() {
        let spec = ContractSpec::ig_defaults().remove("GBPUSD=X").unwrap();
        // 3 bps on 100_000
        assert!((spec.spread_cost(100_000.0) - 30.0).abs() < 1e-10);
    }

    #[test]
    fn spread_cost_equity() {
        let spec = ContractSpec::ig_defaults().remove("SPY").unwrap();
        // 5 bps on 100_000
        assert!((spec.spread_cost(100_000.0) - 50.0).abs() < 1e-10);
    }

    #[test]
    fn spread_cost_gold() {
        let spec = ContractSpec::ig_defaults().remove("GLD").unwrap();
        // 10 bps on 100_000
        assert!((spec.spread_cost(100_000.0) - 100.0).abs() < 1e-10);
    }

    // ── SpreadCostTracker ──────────────────────────────────────

    #[test]
    fn tracker_flat_to_long() {
        let mut t = SpreadCostTracker::new();
        assert_eq!(t.cost_multiplier("SPY", 1.0), 1.0);
    }

    #[test]
    fn tracker_hold() {
        let mut t = SpreadCostTracker::new();
        t.cost_multiplier("SPY", 1.0);
        assert_eq!(t.cost_multiplier("SPY", 1.0), 0.0);
    }

    #[test]
    fn tracker_long_to_flat() {
        let mut t = SpreadCostTracker::new();
        t.cost_multiplier("SPY", 1.0);
        assert_eq!(t.cost_multiplier("SPY", 0.0), 1.0);
    }

    #[test]
    fn tracker_flip() {
        let mut t = SpreadCostTracker::new();
        t.cost_multiplier("SPY", 1.0);
        assert_eq!(t.cost_multiplier("SPY", -1.0), 2.0);
    }

    #[test]
    fn tracker_flat_to_flat() {
        let mut t = SpreadCostTracker::new();
        assert_eq!(t.cost_multiplier("SPY", 0.0), 0.0);
    }

    #[test]
    fn tracker_multi_instrument() {
        let mut t = SpreadCostTracker::new();
        assert_eq!(t.cost_multiplier("SPY", 1.0), 1.0);
        assert_eq!(t.cost_multiplier("GLD", -1.0), 1.0);
        // SPY hold, GLD flip
        assert_eq!(t.cost_multiplier("SPY", 1.0), 0.0);
        assert_eq!(t.cost_multiplier("GLD", 1.0), 2.0);
    }

    // ── ExecutionRouter sizing ─────────────────────────────────

    #[test]
    fn size_from_weight_equity() {
        let router = ExecutionRouter::with_ig_defaults();
        // weight=0.1, nav=100_000, price=500 → notional=10_000, qty=10_000/(500*1)=20
        let qty = router.size_from_weight("SPY", 0.1, 100_000.0, 500.0);
        assert_eq!(qty, 20.0);
    }

    #[test]
    fn size_from_weight_futures() {
        let router = ExecutionRouter::with_ig_defaults();
        // weight=0.1, nav=100_000, price=2000 → notional=10_000, qty=10_000/(2000*100)=0.05 → rounds to 0
        let qty = router.size_from_weight("GC=F", 0.1, 100_000.0, 2000.0);
        assert_eq!(qty, 0.0);
        // Bigger weight: 0.5 → notional=50_000, qty=50_000/(2000*100)=0.25 → rounds to 0 (sub-min)
        let qty2 = router.size_from_weight("GC=F", 0.5, 100_000.0, 2000.0);
        assert_eq!(qty2, 0.0);
        // weight=2.0 → notional=200_000, qty=200_000/(2000*100)=1.0
        let qty3 = router.size_from_weight("GC=F", 2.0, 100_000.0, 2000.0);
        assert_eq!(qty3, 1.0);
    }

    #[test]
    fn size_from_weight_fx() {
        let router = ExecutionRouter::with_ig_defaults();
        // weight=0.1, nav=100_000, price=1.25 → notional=10_000, qty=10_000/(1.25*1)=8000 → rounds to 8000.0
        let qty = router.size_from_weight("GBPUSD=X", 0.1, 100_000.0, 1.25);
        assert_eq!(qty, 8000.0);
    }

    #[test]
    fn size_from_weight_negative() {
        let router = ExecutionRouter::with_ig_defaults();
        let qty = router.size_from_weight("SPY", -0.1, 100_000.0, 500.0);
        assert_eq!(qty, -20.0);
    }

    #[test]
    fn size_from_weight_zero_price() {
        let router = ExecutionRouter::with_ig_defaults();
        assert_eq!(router.size_from_weight("SPY", 0.1, 100_000.0, 0.0), 0.0);
    }

    #[test]
    fn size_from_weight_unknown_fallback() {
        let router = ExecutionRouter::with_ig_defaults();
        // Unknown instrument → default equity (pv=1, lot_step=1, min=1)
        let qty = router.size_from_weight("AAPL", 0.1, 100_000.0, 200.0);
        assert_eq!(qty, 50.0);
    }

    // ── Sized orders ───────────────────────────────────────────

    #[test]
    fn sized_order_buy() {
        let router = ExecutionRouter::with_ig_defaults();
        let order = router.create_sized_order("SPY", 10.0, 0.0, 500.0, 1.0).unwrap();
        assert!(matches!(order.side, OrderSide::Buy));
        assert_eq!(order.quantity, 10.0);
        assert!((order.notional - 5_000.0).abs() < 1e-10);
        assert!((order.margin_required - 1_000.0).abs() < 1e-10);
        // spread: 5_000 * 5/10000 * 1.0 = 2.5
        assert!((order.spread_cost - 2.5).abs() < 1e-10);
    }

    #[test]
    fn sized_order_sell() {
        let router = ExecutionRouter::with_ig_defaults();
        let order = router.create_sized_order("SPY", -5.0, 5.0, 500.0, 1.0).unwrap();
        assert!(matches!(order.side, OrderSide::Sell));
        assert_eq!(order.quantity, 10.0);
    }

    #[test]
    fn sized_order_no_change() {
        let router = ExecutionRouter::with_ig_defaults();
        let order = router.create_sized_order("SPY", 10.0, 10.0, 500.0, 1.0);
        assert!(order.is_none());
    }

    #[test]
    fn sized_order_sub_minimum() {
        let router = ExecutionRouter::with_ig_defaults();
        // Delta = 0.3 for GBPUSD=X rounds to 0.3, below min 0.5
        let order = router.create_sized_order("GBPUSD=X", 5.3, 5.0, 1.25, 1.0);
        assert!(order.is_none());
    }

    #[test]
    fn sized_order_futures() {
        let router = ExecutionRouter::with_ig_defaults();
        let order = router.create_sized_order("GC=F", 2.0, 0.0, 2000.0, 1.0).unwrap();
        assert!(matches!(order.side, OrderSide::Buy));
        assert_eq!(order.quantity, 2.0);
        // notional = 2 * 2000 * 100 = 400_000
        assert!((order.notional - 400_000.0).abs() < 1e-10);
        // margin = 400_000 * 0.05 = 20_000
        assert!((order.margin_required - 20_000.0).abs() < 1e-10);
    }

    #[test]
    fn sized_order_flip_cost_multiplier() {
        let router = ExecutionRouter::with_ig_defaults();
        let order = router.create_sized_order("SPY", 10.0, 0.0, 500.0, 2.0).unwrap();
        // spread cost doubled for flip
        assert!((order.spread_cost - 5.0).abs() < 1e-10);
    }

    // ── Total margin ───────────────────────────────────────────

    #[test]
    fn total_margin() {
        let router = ExecutionRouter::with_ig_defaults();
        let mut targets = HashMap::new();
        targets.insert("SPY".to_string(), 10.0);
        targets.insert("GC=F".to_string(), 1.0);

        let mut prices = HashMap::new();
        prices.insert("SPY".to_string(), 500.0);
        prices.insert("GC=F".to_string(), 2000.0);

        // SPY: 10 * 500 * 1 * 0.20 = 1000
        // GC=F: 1 * 2000 * 100 * 0.05 = 10_000
        let margin = router.total_margin(&targets, &prices);
        assert!((margin - 11_000.0).abs() < 1e-10);
    }
}
