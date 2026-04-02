use serde::{Deserialize, Serialize};

use crate::core::portfolio::OrderSide;

// ─── Error Types ─────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("rate limited")]
    RateLimited,

    #[error("order rejected: {reason} (deal_ref: {deal_reference})")]
    OrderRejected {
        reason: String,
        deal_reference: String,
    },

    #[error("network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("config error: {0}")]
    ConfigError(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

// ─── Shared Types ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DealStatus {
    Accepted,
    Rejected,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePosition {
    pub deal_id: String,
    pub instrument: String,
    pub epic: String,
    pub direction: OrderSide,
    pub size: f64,
    pub open_level: f64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub instrument: String,
    pub epic: String,
    pub direction: OrderSide,
    pub size: f64,
    pub order_type: OrderType,
    pub currency_code: String,
    pub expiry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderAck {
    pub deal_reference: String,
    pub instrument: String,
    pub status: DealStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatus {
    pub deal_reference: String,
    pub deal_id: Option<String>,
    pub status: DealStatus,
    pub reason: Option<String>,
    pub level: Option<f64>,
    pub size: Option<f64>,
}

// ─── Trait ───────────────────────────────────────────────────────

pub trait ExecutionEngine: Send + Sync {
    fn health_check(&self) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;

    fn get_positions(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<LivePosition>, ExecutionError>> + Send;

    fn place_orders(
        &self,
        orders: Vec<OrderRequest>,
    ) -> impl std::future::Future<Output = Result<Vec<OrderAck>, ExecutionError>> + Send;

    fn get_order_status(
        &self,
        deal_refs: &[String],
    ) -> impl std::future::Future<Output = Result<Vec<OrderStatus>, ExecutionError>> + Send;

    fn flatten_all(&self) -> impl std::future::Future<Output = Result<(), ExecutionError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deal_status_serde_round_trip() {
        let statuses = vec![
            DealStatus::Accepted,
            DealStatus::Rejected,
            DealStatus::Pending,
        ];
        let json = serde_json::to_string(&statuses).unwrap();
        let parsed: Vec<DealStatus> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, statuses);
    }

    #[test]
    fn order_request_serde_round_trip() {
        let req = OrderRequest {
            instrument: "SPY".into(),
            epic: "IX.D.SPTRD.DAILY.IP".into(),
            direction: OrderSide::Buy,
            size: 10.0,
            order_type: OrderType::Market,
            currency_code: "GBP".into(),
            expiry: "DFB".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: OrderRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.instrument, "SPY");
        assert_eq!(parsed.size, 10.0);
        assert!(matches!(parsed.direction, OrderSide::Buy));
    }

    #[test]
    fn live_position_serde_round_trip() {
        let pos = LivePosition {
            deal_id: "DEAL123".into(),
            instrument: "GC=F".into(),
            epic: "CC.D.GC.USS.IP".into(),
            direction: OrderSide::Sell,
            size: 2.0,
            open_level: 2050.0,
            currency: "GBP".into(),
        };
        let json = serde_json::to_string(&pos).unwrap();
        let parsed: LivePosition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.deal_id, "DEAL123");
        assert_eq!(parsed.instrument, "GC=F");
        assert!(matches!(parsed.direction, OrderSide::Sell));
    }
}
