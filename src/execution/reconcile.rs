use std::collections::HashMap;

use crate::config::IgConfig;
use crate::core::portfolio::OrderSide;
use crate::execution::traits::{LivePosition, OrderRequest, OrderType};

/// Result of a reconciliation pass.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReconcileResult {
    pub orders: Vec<OrderRequest>,
    pub skipped_dust: Vec<DustDelta>,
    pub unknown_instruments: Vec<String>,
}

/// A delta that was too small to trade (below instrument's min_size).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DustDelta {
    pub instrument: String,
    pub target: f64,
    pub actual: f64,
    pub delta: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PositionMismatch {
    pub instrument: String,
    pub target: f64,
    pub actual: f64,
    pub delta: f64,
}

/// Convert live positions into a signed-size map: instrument → signed quantity.
/// BUY = positive, SELL = negative.
pub fn positions_to_signed(positions: &[LivePosition]) -> HashMap<String, f64> {
    let mut map = HashMap::new();
    for pos in positions {
        let signed = match pos.direction {
            OrderSide::Buy => pos.size,
            OrderSide::Sell => -pos.size,
        };
        // Accumulate in case multiple deals per instrument
        *map.entry(pos.instrument.clone()).or_insert(0.0) += signed;
    }
    map
}

/// Compute delta orders needed to move from actual positions to target quantities.
///
/// Returns a `ReconcileResult` containing:
/// - `orders`: OrderRequests for deltas exceeding per-instrument min_size
/// - `skipped_dust`: deltas too small to trade (tracked for drift monitoring)
/// - `unknown_instruments`: instruments in targets/actuals but missing from config
///
/// The caller should treat `unknown_instruments` as an error condition if any
/// are present in the target set (missing a leg is worse than a failed run).
pub fn compute_deltas(
    target_quantities: &HashMap<String, f64>,
    actual_positions: &HashMap<String, f64>,
    ig_config: &IgConfig,
) -> ReconcileResult {
    let mut orders = Vec::new();
    let mut skipped_dust = Vec::new();
    let mut unknown_instruments = Vec::new();

    // All instruments that appear in either target or actual
    let mut all_instruments: Vec<&String> = target_quantities.keys().collect();
    for sym in actual_positions.keys() {
        if !target_quantities.contains_key(sym) {
            all_instruments.push(sym);
        }
    }
    all_instruments.sort();
    all_instruments.dedup();

    for sym in all_instruments {
        let target = target_quantities.get(sym).copied().unwrap_or(0.0);
        let actual = actual_positions.get(sym).copied().unwrap_or(0.0);
        let delta = target - actual;

        if delta.abs() < 1e-10 {
            continue;
        }

        // Look up instrument config
        let inst_config = match ig_config.instruments.get(sym) {
            Some(c) => c,
            None => {
                unknown_instruments.push(sym.clone());
                continue;
            }
        };

        // Round to step size
        let rounded = (delta.abs() / inst_config.size_step).round() * inst_config.size_step;

        // Track dust: delta exists but rounds below minimum deal size
        if rounded < inst_config.min_size {
            skipped_dust.push(DustDelta {
                instrument: sym.clone(),
                target,
                actual,
                delta,
            });
            continue;
        }

        let direction = if delta > 0.0 {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };

        orders.push(OrderRequest {
            instrument: sym.clone(),
            epic: inst_config.epic.clone(),
            direction,
            size: rounded,
            order_type: OrderType::Market,
            currency_code: inst_config.currency().to_string(),
            expiry: inst_config.expiry().to_string(),
        });
    }

    ReconcileResult {
        orders,
        skipped_dust,
        unknown_instruments,
    }
}

/// Post-trade verification: compare actual positions against targets.
///
/// Returns mismatches where `abs(target - actual) >= tolerance`.
/// Tolerance is the instrument's `min_size` — deltas below this are
/// acceptable dust that can't be traded due to IG's minimum deal size.
///
/// Dust deltas (from `compute_deltas`) are expected to appear as small
/// mismatches here and are correctly tolerated.
pub fn verify_positions(
    target_quantities: &HashMap<String, f64>,
    actual_positions: &HashMap<String, f64>,
    ig_config: &IgConfig,
) -> Vec<PositionMismatch> {
    let mut mismatches = Vec::new();

    let mut all_instruments: Vec<&String> = target_quantities.keys().collect();
    for sym in actual_positions.keys() {
        if !target_quantities.contains_key(sym) {
            all_instruments.push(sym);
        }
    }
    all_instruments.sort();
    all_instruments.dedup();

    for sym in all_instruments {
        let target = target_quantities.get(sym).copied().unwrap_or(0.0);
        let actual = actual_positions.get(sym).copied().unwrap_or(0.0);
        let delta = (target - actual).abs();

        // Tolerance = instrument's min_size (smallest tradeable unit).
        // Deltas below this are "dust" — we can't correct them, so they're acceptable.
        let tolerance = ig_config
            .instruments
            .get(sym)
            .map(|c| c.min_size)
            .unwrap_or(0.1);

        if delta >= tolerance {
            mismatches.push(PositionMismatch {
                instrument: sym.clone(),
                target,
                actual,
                delta: target - actual,
            });
        }
    }

    mismatches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IgEnvironment, InstrumentConfig};

    fn test_ig_config() -> IgConfig {
        let mut instruments = HashMap::new();
        instruments.insert(
            "GBPUSD=X".to_string(),
            InstrumentConfig {
                epic: "CS.D.GBPUSD.TODAY.IP".to_string(),
                min_size: 0.5,
                size_step: 0.1,
                currency_code: None,
                expiry: None,
            },
        );
        instruments.insert(
            "SPY".to_string(),
            InstrumentConfig {
                epic: "IX.D.SPTRD.DAILY.IP".to_string(),
                min_size: 1.0,
                size_step: 1.0,
                currency_code: None,
                expiry: None,
            },
        );
        instruments.insert(
            "GLD".to_string(),
            InstrumentConfig {
                epic: "UC.D.GLDUS.DAILY.IP".to_string(),
                min_size: 1.0,
                size_step: 1.0,
                currency_code: None,
                expiry: None,
            },
        );
        IgConfig {
            environment: IgEnvironment::Demo,
            account_id: "TEST".to_string(),
            instruments,
        }
    }

    #[test]
    fn target_equals_actual_no_orders() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("GBPUSD=X".to_string(), 100.0);

        let mut actuals = HashMap::new();
        actuals.insert("GBPUSD=X".to_string(), 100.0);

        let result = compute_deltas(&targets, &actuals, &config);
        assert!(result.orders.is_empty());
        assert!(result.skipped_dust.is_empty());
        assert!(result.unknown_instruments.is_empty());
    }

    #[test]
    fn new_position_creates_buy() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("GBPUSD=X".to_string(), 10.0);

        let result = compute_deltas(&targets, &HashMap::new(), &config);
        assert_eq!(result.orders.len(), 1);
        assert_eq!(result.orders[0].instrument, "GBPUSD=X");
        assert!(matches!(result.orders[0].direction, OrderSide::Buy));
        assert_eq!(result.orders[0].size, 10.0);
    }

    #[test]
    fn stale_position_creates_sell() {
        let config = test_ig_config();
        let mut actuals = HashMap::new();
        actuals.insert("SPY".to_string(), 5.0);

        let result = compute_deltas(&HashMap::new(), &actuals, &config);
        assert_eq!(result.orders.len(), 1);
        assert_eq!(result.orders[0].instrument, "SPY");
        assert!(matches!(result.orders[0].direction, OrderSide::Sell));
        assert_eq!(result.orders[0].size, 5.0);
    }

    #[test]
    fn small_delta_tracked_as_dust() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("GBPUSD=X".to_string(), 10.0);

        let mut actuals = HashMap::new();
        actuals.insert("GBPUSD=X".to_string(), 9.8); // delta 0.2 < min 0.5

        let result = compute_deltas(&targets, &actuals, &config);
        assert!(result.orders.is_empty());
        assert_eq!(result.skipped_dust.len(), 1);
        assert_eq!(result.skipped_dust[0].instrument, "GBPUSD=X");
        assert!((result.skipped_dust[0].delta - 0.2).abs() < 1e-10);
    }

    #[test]
    fn unknown_instrument_reported() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("UNKNOWN".to_string(), 10.0);

        let result = compute_deltas(&targets, &HashMap::new(), &config);
        assert!(result.orders.is_empty());
        assert_eq!(result.unknown_instruments, vec!["UNKNOWN"]);
    }

    #[test]
    fn delta_rounded_to_step() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("GBPUSD=X".to_string(), 10.73);

        let mut actuals = HashMap::new();
        actuals.insert("GBPUSD=X".to_string(), 10.0);

        let result = compute_deltas(&targets, &actuals, &config);
        assert_eq!(result.orders.len(), 1);
        assert!((result.orders[0].size - 0.7).abs() < 1e-10);
    }

    #[test]
    fn short_position_handling() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("SPY".to_string(), -5.0);

        let result = compute_deltas(&targets, &HashMap::new(), &config);
        assert_eq!(result.orders.len(), 1);
        assert!(matches!(result.orders[0].direction, OrderSide::Sell));
        assert_eq!(result.orders[0].size, 5.0);
    }

    #[test]
    fn flip_long_to_short() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("SPY".to_string(), -3.0);

        let mut actuals = HashMap::new();
        actuals.insert("SPY".to_string(), 5.0);

        let result = compute_deltas(&targets, &actuals, &config);
        assert_eq!(result.orders.len(), 1);
        assert!(matches!(result.orders[0].direction, OrderSide::Sell));
        assert_eq!(result.orders[0].size, 8.0);
    }

    #[test]
    fn positions_to_signed_buy() {
        let positions = vec![LivePosition {
            deal_id: "D1".into(),
            instrument: "SPY".into(),
            epic: "IX.D.SPTRD.DAILY.IP".into(),
            direction: OrderSide::Buy,
            size: 10.0,
            open_level: 500.0,
            currency: "GBP".into(),
        }];
        let map = positions_to_signed(&positions);
        assert_eq!(map["SPY"], 10.0);
    }

    #[test]
    fn positions_to_signed_sell() {
        let positions = vec![LivePosition {
            deal_id: "D1".into(),
            instrument: "SPY".into(),
            epic: "IX.D.SPTRD.DAILY.IP".into(),
            direction: OrderSide::Sell,
            size: 5.0,
            open_level: 500.0,
            currency: "GBP".into(),
        }];
        let map = positions_to_signed(&positions);
        assert_eq!(map["SPY"], -5.0);
    }

    #[test]
    fn verify_positions_within_tolerance() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("GBPUSD=X".to_string(), 10.0);

        let mut actuals = HashMap::new();
        actuals.insert("GBPUSD=X".to_string(), 10.3); // delta 0.3 < min_size 0.5

        let mismatches = verify_positions(&targets, &actuals, &config);
        assert!(mismatches.is_empty());
    }

    #[test]
    fn verify_positions_mismatch() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("SPY".to_string(), 10.0);

        let mut actuals = HashMap::new();
        actuals.insert("SPY".to_string(), 7.0);

        let mismatches = verify_positions(&targets, &actuals, &config);
        assert_eq!(mismatches.len(), 1);
        assert_eq!(mismatches[0].instrument, "SPY");
        assert_eq!(mismatches[0].target, 10.0);
        assert_eq!(mismatches[0].actual, 7.0);
    }

    #[test]
    fn multiple_instruments_mixed() {
        let config = test_ig_config();
        let mut targets = HashMap::new();
        targets.insert("GBPUSD=X".to_string(), 100.0);
        targets.insert("SPY".to_string(), -10.0);
        targets.insert("GLD".to_string(), 50.0);

        let mut actuals = HashMap::new();
        actuals.insert("GBPUSD=X".to_string(), 100.0);
        actuals.insert("SPY".to_string(), -8.0);

        let result = compute_deltas(&targets, &actuals, &config);
        assert_eq!(result.orders.len(), 2);

        let gld_order = result.orders.iter().find(|o| o.instrument == "GLD").unwrap();
        assert!(matches!(gld_order.direction, OrderSide::Buy));
        assert_eq!(gld_order.size, 50.0);

        let spy_order = result.orders.iter().find(|o| o.instrument == "SPY").unwrap();
        assert!(matches!(spy_order.direction, OrderSide::Sell));
        assert_eq!(spy_order.size, 2.0);
    }

    #[test]
    fn dust_does_not_cause_verify_mismatch() {
        let config = test_ig_config();
        // Target 10.2 but we can only hold 10.0 (dust delta 0.2 < min 0.5)
        let mut targets = HashMap::new();
        targets.insert("GBPUSD=X".to_string(), 10.2);

        let mut actuals = HashMap::new();
        actuals.insert("GBPUSD=X".to_string(), 10.0);

        // compute_deltas should report dust
        let result = compute_deltas(&targets, &actuals, &config);
        assert!(result.orders.is_empty());
        assert_eq!(result.skipped_dust.len(), 1);

        // verify_positions should NOT report mismatch (within tolerance)
        let mismatches = verify_positions(&targets, &actuals, &config);
        assert!(mismatches.is_empty());
    }
}
