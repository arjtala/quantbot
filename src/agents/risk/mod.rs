use serde::{Deserialize, Serialize};

use crate::execution::router::SizedOrder;

// ─── Configuration ──────────────────────────────────────────────

/// Risk limits for the hard-veto RiskAgent.
///
/// These act as a second line of defense beyond the BacktestConfig limits
/// applied during signal generation. If any check fails, the entire run
/// is vetoed and no orders are placed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Maximum gross leverage (sum of |notional| / NAV). Should be slightly
    /// above BacktestConfig::max_gross_leverage to allow for rounding.
    #[serde(default = "default_max_gross_leverage")]
    pub max_gross_leverage: f64,

    /// Maximum single-instrument exposure as fraction of NAV.
    #[serde(default = "default_max_position_pct")]
    pub max_position_pct: f64,

    /// Maximum drawdown from peak NAV before vetoing new trades (e.g. 0.15 = 15%).
    #[serde(default = "default_max_drawdown_pct")]
    pub max_drawdown_pct: f64,
}

fn default_max_gross_leverage() -> f64 {
    2.5
}
fn default_max_position_pct() -> f64 {
    0.25
}
fn default_max_drawdown_pct() -> f64 {
    0.15
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_gross_leverage: default_max_gross_leverage(),
            max_position_pct: default_max_position_pct(),
            max_drawdown_pct: default_max_drawdown_pct(),
        }
    }
}

// ─── Decision ───────────────────────────────────────────────────

/// Outcome of a risk check.
#[derive(Debug, Clone, PartialEq)]
pub enum RiskDecision {
    Allow,
    Veto { reason: String },
}

// ─── Risk Check Detail ─────────────────────────────────────────

/// Detailed breakdown of a risk check, for logging.
#[derive(Debug, Clone, Serialize)]
pub struct RiskCheckDetail {
    pub gross_leverage: f64,
    pub max_position_leverage: f64,
    pub max_position_instrument: String,
    pub drawdown_pct: f64,
    pub peak_nav: f64,
    pub current_nav: f64,
    pub decision: String,
    pub reason: Option<String>,
}

// ─── RiskAgent ──────────────────────────────────────────────────

/// Hard-veto risk agent. Checks proposed target positions against
/// configured limits and returns Allow or Veto.
pub struct RiskAgent {
    config: RiskConfig,
}

impl RiskAgent {
    pub fn new(config: RiskConfig) -> Self {
        Self { config }
    }

    /// Check target positions against exposure limits.
    ///
    /// `orders` should be the full target positions expressed as orders from flat
    /// (as produced by `generate_targets` with empty current quantities).
    /// Each `SizedOrder` must have `notional` already computed.
    pub fn check_targets(&self, orders: &[SizedOrder], nav: f64) -> RiskDecision {
        if nav <= 0.0 {
            return RiskDecision::Veto {
                reason: format!("NAV is non-positive ({nav:.0})"),
            };
        }

        // Gross leverage = sum(|notional|) / NAV
        let gross_notional: f64 = orders.iter().map(|o| o.notional.abs()).sum();
        let gross_leverage = gross_notional / nav;

        if gross_leverage > self.config.max_gross_leverage {
            return RiskDecision::Veto {
                reason: format!(
                    "gross leverage {gross_leverage:.2} exceeds limit {:.2}",
                    self.config.max_gross_leverage
                ),
            };
        }

        // Per-instrument check
        for order in orders {
            let position_pct = order.notional.abs() / nav;
            if position_pct > self.config.max_position_pct {
                return RiskDecision::Veto {
                    reason: format!(
                        "{} position {:.1}% exceeds limit {:.1}%",
                        order.instrument,
                        position_pct * 100.0,
                        self.config.max_position_pct * 100.0,
                    ),
                };
            }
        }

        RiskDecision::Allow
    }

    /// Check drawdown against limit.
    ///
    /// Drawdown is computed as `(current_nav - peak_nav) / peak_nav`.
    /// Returns Veto if drawdown exceeds `max_drawdown_pct`.
    pub fn check_drawdown(&self, current_nav: f64, peak_nav: f64) -> RiskDecision {
        if peak_nav <= 0.0 {
            // Can't compute drawdown, allow but this shouldn't happen
            return RiskDecision::Allow;
        }

        let drawdown = (current_nav - peak_nav) / peak_nav;

        if drawdown < -self.config.max_drawdown_pct {
            return RiskDecision::Veto {
                reason: format!(
                    "drawdown {:.1}% exceeds limit {:.1}% (NAV {current_nav:.0} vs peak {peak_nav:.0})",
                    drawdown * 100.0,
                    self.config.max_drawdown_pct * 100.0,
                ),
            };
        }

        RiskDecision::Allow
    }

    /// Run all risk checks and return a combined decision with detail.
    pub fn check_all(
        &self,
        orders: &[SizedOrder],
        nav: f64,
        peak_nav: f64,
    ) -> (RiskDecision, RiskCheckDetail) {
        // Compute metrics for logging
        let gross_notional: f64 = orders.iter().map(|o| o.notional.abs()).sum();
        let gross_leverage = if nav > 0.0 {
            gross_notional / nav
        } else {
            f64::INFINITY
        };

        let (max_pos_leverage, max_pos_instrument) = orders
            .iter()
            .map(|o| (o.notional.abs() / nav.max(1.0), o.instrument.clone()))
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((0.0, String::new()));

        let drawdown = if peak_nav > 0.0 {
            (nav - peak_nav) / peak_nav
        } else {
            0.0
        };

        // Run checks in order
        let decision = self.check_targets(orders, nav);
        let decision = if decision == RiskDecision::Allow {
            self.check_drawdown(nav, peak_nav)
        } else {
            decision
        };

        let (decision_str, reason) = match &decision {
            RiskDecision::Allow => ("ALLOW".to_string(), None),
            RiskDecision::Veto { reason } => ("VETO".to_string(), Some(reason.clone())),
        };

        let detail = RiskCheckDetail {
            gross_leverage,
            max_position_leverage: max_pos_leverage,
            max_position_instrument: max_pos_instrument,
            drawdown_pct: drawdown,
            peak_nav,
            current_nav: nav,
            decision: decision_str,
            reason,
        };

        (decision, detail)
    }
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::portfolio::OrderSide;

    fn make_order(instrument: &str, notional: f64) -> SizedOrder {
        SizedOrder {
            instrument: instrument.to_string(),
            side: if notional >= 0.0 {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            },
            quantity: notional.abs() / 100.0, // dummy
            reference_price: 100.0,
            point_value: 1.0,
            notional,
            margin_required: notional.abs() * 0.2,
            spread_cost: 0.0,
        }
    }

    #[test]
    fn allow_within_limits() {
        let agent = RiskAgent::new(RiskConfig::default());
        let orders = vec![make_order("SPY", -100_000.0), make_order("GLD", 150_000.0)];
        // Gross leverage = 250k / 1M = 0.25, well under 2.5
        let decision = agent.check_targets(&orders, 1_000_000.0);
        assert_eq!(decision, RiskDecision::Allow);
    }

    #[test]
    fn veto_gross_leverage() {
        let agent = RiskAgent::new(RiskConfig {
            max_gross_leverage: 2.0,
            ..Default::default()
        });
        let orders = vec![
            make_order("SPY", -1_200_000.0),
            make_order("GLD", 1_000_000.0),
        ];
        // Gross = 2.2M / 1M = 2.2 > 2.0
        let decision = agent.check_targets(&orders, 1_000_000.0);
        assert!(matches!(decision, RiskDecision::Veto { .. }));
        if let RiskDecision::Veto { reason } = decision {
            assert!(reason.contains("gross leverage"));
        }
    }

    #[test]
    fn veto_per_instrument() {
        let agent = RiskAgent::new(RiskConfig {
            max_position_pct: 0.20,
            ..Default::default()
        });
        let orders = vec![
            make_order("SPY", -100_000.0), // 10% — ok
            make_order("GLD", 250_000.0),  // 25% — over 20%
        ];
        let decision = agent.check_targets(&orders, 1_000_000.0);
        assert!(matches!(decision, RiskDecision::Veto { .. }));
        if let RiskDecision::Veto { reason } = decision {
            assert!(reason.contains("GLD"));
        }
    }

    #[test]
    fn allow_no_drawdown() {
        let agent = RiskAgent::new(RiskConfig::default());
        // NAV at peak — no drawdown
        let decision = agent.check_drawdown(1_000_000.0, 1_000_000.0);
        assert_eq!(decision, RiskDecision::Allow);
    }

    #[test]
    fn allow_small_drawdown() {
        let agent = RiskAgent::new(RiskConfig {
            max_drawdown_pct: 0.15,
            ..Default::default()
        });
        // 10% drawdown, under 15% limit
        let decision = agent.check_drawdown(900_000.0, 1_000_000.0);
        assert_eq!(decision, RiskDecision::Allow);
    }

    #[test]
    fn veto_drawdown() {
        let agent = RiskAgent::new(RiskConfig {
            max_drawdown_pct: 0.15,
            ..Default::default()
        });
        // 20% drawdown, over 15% limit
        let decision = agent.check_drawdown(800_000.0, 1_000_000.0);
        assert!(matches!(decision, RiskDecision::Veto { .. }));
        if let RiskDecision::Veto { reason } = decision {
            assert!(reason.contains("drawdown"));
        }
    }

    #[test]
    fn veto_zero_nav() {
        let agent = RiskAgent::new(RiskConfig::default());
        let orders = vec![make_order("SPY", 100_000.0)];
        let decision = agent.check_targets(&orders, 0.0);
        assert!(matches!(decision, RiskDecision::Veto { .. }));
    }

    #[test]
    fn check_all_combines_checks() {
        let agent = RiskAgent::new(RiskConfig {
            max_gross_leverage: 2.0,
            max_position_pct: 0.25,
            max_drawdown_pct: 0.15,
        });
        let orders = vec![make_order("SPY", -100_000.0), make_order("GLD", 150_000.0)];
        // All within limits, no drawdown
        let (decision, detail) = agent.check_all(&orders, 1_000_000.0, 1_000_000.0);
        assert_eq!(decision, RiskDecision::Allow);
        assert_eq!(detail.decision, "ALLOW");
        assert!((detail.gross_leverage - 0.25).abs() < 0.01);
        assert!(detail.reason.is_none());
    }

    #[test]
    fn check_all_veto_reports_detail() {
        let agent = RiskAgent::new(RiskConfig {
            max_gross_leverage: 2.0,
            max_position_pct: 0.25,
            max_drawdown_pct: 0.10,
        });
        let orders = vec![make_order("SPY", -100_000.0)];
        // Targets are fine, but drawdown is 12% > 10% limit
        let (decision, detail) = agent.check_all(&orders, 880_000.0, 1_000_000.0);
        assert!(matches!(decision, RiskDecision::Veto { .. }));
        assert_eq!(detail.decision, "VETO");
        assert!(detail.reason.unwrap().contains("drawdown"));
    }

    #[test]
    fn empty_orders_allowed() {
        let agent = RiskAgent::new(RiskConfig::default());
        let decision = agent.check_targets(&[], 1_000_000.0);
        assert_eq!(decision, RiskDecision::Allow);
    }
}
