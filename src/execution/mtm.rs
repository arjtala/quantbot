use std::collections::HashMap;

use serde::Serialize;

/// Per-position mark-to-market breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct MtmPosition {
    pub instrument: String,
    pub signed_size: f64,
    pub open_level: f64,
    pub current_price: f64,
    pub pnl: f64,
}

/// Result of a mark-to-market NAV computation.
#[derive(Debug, Clone, Serialize)]
pub struct MtmResult {
    pub nav: f64,
    pub unrealized_pnl: f64,
    pub positions: Vec<MtmPosition>,
}

/// Compute NAV from initial cash plus unrealized P&L on open positions.
///
/// For IG spread betting:
///   pnl_per_position = signed_size × (current_price - open_level)
///   mtm_nav = initial_cash + Σ pnl
///
/// Positions missing a current price or open level are skipped (0 P&L) with a
/// warning to stderr.
pub fn mark_to_market(
    initial_cash: f64,
    positions: &HashMap<String, f64>,
    open_levels: &HashMap<String, f64>,
    current_prices: &HashMap<String, f64>,
) -> MtmResult {
    let mut unrealized_pnl = 0.0;
    let mut mtm_positions = Vec::new();

    let mut instruments: Vec<&String> = positions.keys().collect();
    instruments.sort();

    for sym in instruments {
        let signed_size = positions[sym];

        let open_level = match open_levels.get(sym) {
            Some(&v) => v,
            None => {
                eprintln!("  WARN: no open_level for {sym} — skipping MTM P&L");
                mtm_positions.push(MtmPosition {
                    instrument: sym.clone(),
                    signed_size,
                    open_level: 0.0,
                    current_price: 0.0,
                    pnl: 0.0,
                });
                continue;
            }
        };

        let current_price = match current_prices.get(sym) {
            Some(&v) => v,
            None => {
                eprintln!("  WARN: no current_price for {sym} — skipping MTM P&L");
                mtm_positions.push(MtmPosition {
                    instrument: sym.clone(),
                    signed_size,
                    open_level,
                    current_price: 0.0,
                    pnl: 0.0,
                });
                continue;
            }
        };

        let pnl = signed_size * (current_price - open_level);
        unrealized_pnl += pnl;

        mtm_positions.push(MtmPosition {
            instrument: sym.clone(),
            signed_size,
            open_level,
            current_price,
            pnl,
        });
    }

    MtmResult {
        nav: initial_cash + unrealized_pnl,
        unrealized_pnl,
        positions: mtm_positions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_positions_returns_initial_cash() {
        let result = mark_to_market(
            1_000_000.0,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert!((result.nav - 1_000_000.0).abs() < 1e-10);
        assert!((result.unrealized_pnl - 0.0).abs() < 1e-10);
        assert!(result.positions.is_empty());
    }

    #[test]
    fn long_position_price_up() {
        let mut positions = HashMap::new();
        positions.insert("SPY".to_string(), 10.0); // long 10

        let mut open_levels = HashMap::new();
        open_levels.insert("SPY".to_string(), 500.0);

        let mut current_prices = HashMap::new();
        current_prices.insert("SPY".to_string(), 510.0); // up 10 points

        let result = mark_to_market(1_000_000.0, &positions, &open_levels, &current_prices);

        // pnl = 10 * (510 - 500) = 100
        assert!((result.unrealized_pnl - 100.0).abs() < 1e-10);
        assert!((result.nav - 1_000_100.0).abs() < 1e-10);
        assert_eq!(result.positions.len(), 1);
        assert!((result.positions[0].pnl - 100.0).abs() < 1e-10);
    }

    #[test]
    fn short_position_price_down() {
        let mut positions = HashMap::new();
        positions.insert("GBPUSD=X".to_string(), -5.0); // short 5

        let mut open_levels = HashMap::new();
        open_levels.insert("GBPUSD=X".to_string(), 1.2700);

        let mut current_prices = HashMap::new();
        current_prices.insert("GBPUSD=X".to_string(), 1.2600); // down 0.01

        let result = mark_to_market(1_000_000.0, &positions, &open_levels, &current_prices);

        // pnl = -5 * (1.26 - 1.27) = -5 * -0.01 = 0.05
        assert!((result.unrealized_pnl - 0.05).abs() < 1e-10);
        assert!((result.nav - 1_000_000.05).abs() < 1e-10);
    }

    #[test]
    fn mixed_positions() {
        let mut positions = HashMap::new();
        positions.insert("SPY".to_string(), 10.0);
        positions.insert("GBPUSD=X".to_string(), -5.0);

        let mut open_levels = HashMap::new();
        open_levels.insert("SPY".to_string(), 500.0);
        open_levels.insert("GBPUSD=X".to_string(), 1.2700);

        let mut current_prices = HashMap::new();
        current_prices.insert("SPY".to_string(), 490.0); // down 10
        current_prices.insert("GBPUSD=X".to_string(), 1.2600); // down 0.01

        let result = mark_to_market(1_000_000.0, &positions, &open_levels, &current_prices);

        // SPY pnl = 10 * (490 - 500) = -100
        // GBPUSD pnl = -5 * (1.26 - 1.27) = +0.05
        let expected_pnl = -100.0 + 0.05;
        assert!((result.unrealized_pnl - expected_pnl).abs() < 1e-10);
        assert!((result.nav - (1_000_000.0 + expected_pnl)).abs() < 1e-10);
        assert_eq!(result.positions.len(), 2);
    }

    #[test]
    fn missing_current_price_skipped() {
        let mut positions = HashMap::new();
        positions.insert("SPY".to_string(), 10.0);

        let mut open_levels = HashMap::new();
        open_levels.insert("SPY".to_string(), 500.0);

        // No current price for SPY
        let result = mark_to_market(1_000_000.0, &positions, &open_levels, &HashMap::new());

        // Missing price → 0 pnl for that position
        assert!((result.unrealized_pnl - 0.0).abs() < 1e-10);
        assert!((result.nav - 1_000_000.0).abs() < 1e-10);
        assert_eq!(result.positions.len(), 1);
        assert!((result.positions[0].pnl - 0.0).abs() < 1e-10);
    }

    #[test]
    fn missing_open_level_skipped() {
        let mut positions = HashMap::new();
        positions.insert("SPY".to_string(), 10.0);

        let mut current_prices = HashMap::new();
        current_prices.insert("SPY".to_string(), 510.0);

        // No open level
        let result = mark_to_market(1_000_000.0, &positions, &HashMap::new(), &current_prices);

        assert!((result.unrealized_pnl - 0.0).abs() < 1e-10);
        assert!((result.nav - 1_000_000.0).abs() < 1e-10);
    }
}
