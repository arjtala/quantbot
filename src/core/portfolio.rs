use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub instrument: String,
    pub quantity: f64,
    pub avg_entry_price: f64,
    pub point_value: f64,
}

impl Position {
    pub fn new(instrument: String, quantity: f64, avg_entry_price: f64) -> Self {
        Self {
            instrument,
            quantity,
            avg_entry_price,
            point_value: 1.0,
        }
    }

    pub fn notional(&self) -> f64 {
        self.quantity.abs() * self.avg_entry_price * self.point_value
    }

    pub fn unrealised_pnl(&self, current_price: f64) -> f64 {
        self.quantity * (current_price - self.avg_entry_price) * self.point_value
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub instrument: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub timestamp: DateTime<Utc>,
    pub limit_price: Option<f64>,
}

impl Order {
    pub fn new(instrument: String, side: OrderSide, quantity: f64) -> Self {
        Self {
            instrument,
            side,
            quantity,
            timestamp: Utc::now(),
            limit_price: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub order: Order,
    pub fill_price: f64,
    pub fill_quantity: f64,
    pub timestamp: DateTime<Utc>,
    pub slippage_bps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioState {
    pub cash: f64,
    pub positions: HashMap<String, Position>,
    pub timestamp: DateTime<Utc>,
}

impl PortfolioState {
    pub fn new(cash: f64) -> Self {
        Self {
            cash,
            positions: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    pub fn nav(&self) -> f64 {
        let pos_value: f64 = self
            .positions
            .values()
            .map(|p| p.quantity * p.avg_entry_price * p.point_value)
            .sum();
        self.cash + pos_value
    }

    pub fn gross_exposure(&self, prices: Option<&HashMap<String, f64>>) -> f64 {
        self.positions
            .iter()
            .map(|(sym, pos)| {
                let px = prices
                    .and_then(|p| p.get(sym))
                    .copied()
                    .unwrap_or(pos.avg_entry_price);
                pos.quantity.abs() * px * pos.point_value
            })
            .sum()
    }

    pub fn net_exposure(&self, prices: Option<&HashMap<String, f64>>) -> f64 {
        self.positions
            .iter()
            .map(|(sym, pos)| {
                let px = prices
                    .and_then(|p| p.get(sym))
                    .copied()
                    .unwrap_or(pos.avg_entry_price);
                pos.quantity * px * pos.point_value
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_notional_and_pnl() {
        let pos = Position {
            instrument: "GC=F".into(),
            quantity: 2.0,
            avg_entry_price: 2000.0,
            point_value: 100.0,
        };
        assert!((pos.notional() - 400_000.0).abs() < 1e-10);
        assert!((pos.unrealised_pnl(2050.0) - 10_000.0).abs() < 1e-10);
    }

    #[test]
    fn short_position_pnl() {
        let pos = Position {
            instrument: "SPY".into(),
            quantity: -10.0,
            avg_entry_price: 500.0,
            point_value: 1.0,
        };
        // Price dropped to 490 → profit for short
        assert!((pos.unrealised_pnl(490.0) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn portfolio_nav_and_exposure() {
        let mut state = PortfolioState::new(100_000.0);
        state
            .positions
            .insert("SPY".into(), Position::new("SPY".into(), 10.0, 500.0));
        state.positions.insert(
            "GLD".into(),
            Position {
                instrument: "GLD".into(),
                quantity: -5.0,
                avg_entry_price: 200.0,
                point_value: 1.0,
            },
        );

        // NAV = 100_000 + (10*500) + (-5*200) = 100_000 + 5000 - 1000 = 104_000
        assert!((state.nav() - 104_000.0).abs() < 1e-10);

        // Gross exposure without prices: |10|*500 + |-5|*200 = 5000 + 1000 = 6000
        assert!((state.gross_exposure(None) - 6_000.0).abs() < 1e-10);

        // Net exposure without prices: 10*500 + (-5)*200 = 5000 - 1000 = 4000
        assert!((state.net_exposure(None) - 4_000.0).abs() < 1e-10);
    }
}
