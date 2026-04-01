use std::sync::Mutex;

use crate::core::portfolio::OrderSide;
use crate::execution::traits::{
    DealStatus, ExecutionEngine, ExecutionError, LivePosition, OrderAck, OrderRequest, OrderStatus,
};

/// Paper execution engine that simulates instant fills with no network calls.
pub struct PaperExecutionEngine {
    positions: Mutex<Vec<LivePosition>>,
    deal_counter: Mutex<u64>,
}

impl PaperExecutionEngine {
    pub fn new() -> Self {
        Self {
            positions: Mutex::new(Vec::new()),
            deal_counter: Mutex::new(0),
        }
    }

    fn next_deal_id(&self) -> String {
        let mut counter = self.deal_counter.lock().unwrap();
        *counter += 1;
        format!("PAPER-{counter}")
    }
}

impl Default for PaperExecutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionEngine for PaperExecutionEngine {
    async fn health_check(&self) -> Result<(), ExecutionError> {
        Ok(())
    }

    async fn get_positions(&self) -> Result<Vec<LivePosition>, ExecutionError> {
        let positions = self.positions.lock().unwrap();
        Ok(positions.clone())
    }

    async fn place_orders(
        &self,
        orders: Vec<OrderRequest>,
    ) -> Result<Vec<OrderAck>, ExecutionError> {
        let mut positions = self.positions.lock().unwrap();
        let mut acks = Vec::with_capacity(orders.len());

        for order in orders {
            let deal_id = self.next_deal_id();

            // Check if there's an existing position for this instrument
            let existing_idx = positions
                .iter()
                .position(|p| p.instrument == order.instrument);

            match existing_idx {
                Some(idx) => {
                    let signed_order_size = match order.direction {
                        OrderSide::Buy => order.size,
                        OrderSide::Sell => -order.size,
                    };
                    let existing_signed = match positions[idx].direction {
                        OrderSide::Buy => positions[idx].size,
                        OrderSide::Sell => -positions[idx].size,
                    };
                    let new_signed = existing_signed + signed_order_size;

                    if new_signed.abs() < 1e-10 {
                        positions.remove(idx);
                    } else {
                        positions[idx].size = new_signed.abs();
                        positions[idx].direction = if new_signed > 0.0 {
                            OrderSide::Buy
                        } else {
                            OrderSide::Sell
                        };
                        positions[idx].deal_id = deal_id.clone();
                    }
                }
                None => {
                    positions.push(LivePosition {
                        deal_id: deal_id.clone(),
                        instrument: order.instrument.clone(),
                        epic: order.epic.clone(),
                        direction: order.direction,
                        size: order.size,
                        open_level: 0.0,
                        currency: order.currency_code.clone(),
                    });
                }
            }

            acks.push(OrderAck {
                deal_reference: deal_id,
                instrument: order.instrument,
                status: DealStatus::Accepted,
            });
        }

        Ok(acks)
    }

    async fn get_order_status(
        &self,
        deal_refs: &[String],
    ) -> Result<Vec<OrderStatus>, ExecutionError> {
        Ok(deal_refs
            .iter()
            .map(|r| OrderStatus {
                deal_reference: r.clone(),
                deal_id: Some(r.clone()),
                status: DealStatus::Accepted,
                reason: None,
                level: None,
                size: None,
            })
            .collect())
    }

    async fn flatten_all(&self) -> Result<(), ExecutionError> {
        let mut positions = self.positions.lock().unwrap();
        positions.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_check_ok() {
        let engine = PaperExecutionEngine::new();
        assert!(engine.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn place_and_get_positions() {
        let engine = PaperExecutionEngine::new();

        let orders = vec![
            OrderRequest {
                instrument: "SPY".into(),
                epic: "IX.D.SPTRD.DAILY.IP".into(),
                direction: OrderSide::Buy,
                size: 10.0,
                order_type: crate::execution::traits::OrderType::Market,
                currency_code: "GBP".into(),
                expiry: "DFB".into(),
            },
            OrderRequest {
                instrument: "GLD".into(),
                epic: "UC.D.GLDUS.DAILY.IP".into(),
                direction: OrderSide::Sell,
                size: 5.0,
                order_type: crate::execution::traits::OrderType::Market,
                currency_code: "GBP".into(),
                expiry: "DFB".into(),
            },
        ];

        let acks = engine.place_orders(orders).await.unwrap();
        assert_eq!(acks.len(), 2);
        assert!(acks.iter().all(|a| a.status == DealStatus::Accepted));

        let positions = engine.get_positions().await.unwrap();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].instrument, "SPY");
        assert_eq!(positions[0].size, 10.0);
        assert!(matches!(positions[0].direction, OrderSide::Buy));
        assert_eq!(positions[1].instrument, "GLD");
        assert!(matches!(positions[1].direction, OrderSide::Sell));
    }

    #[tokio::test]
    async fn flatten_clears_positions() {
        let engine = PaperExecutionEngine::new();

        let orders = vec![OrderRequest {
            instrument: "SPY".into(),
            epic: "IX.D.SPTRD.DAILY.IP".into(),
            direction: OrderSide::Buy,
            size: 10.0,
            order_type: crate::execution::traits::OrderType::Market,
            currency_code: "GBP".into(),
            expiry: "DFB".into(),
        }];

        engine.place_orders(orders).await.unwrap();
        assert_eq!(engine.get_positions().await.unwrap().len(), 1);

        engine.flatten_all().await.unwrap();
        assert!(engine.get_positions().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn close_position_by_opposite_order() {
        let engine = PaperExecutionEngine::new();

        // Open long
        engine
            .place_orders(vec![OrderRequest {
                instrument: "SPY".into(),
                epic: "IX.D.SPTRD.DAILY.IP".into(),
                direction: OrderSide::Buy,
                size: 10.0,
                order_type: crate::execution::traits::OrderType::Market,
                currency_code: "GBP".into(),
                expiry: "DFB".into(),
            }])
            .await
            .unwrap();

        // Close by selling same size
        engine
            .place_orders(vec![OrderRequest {
                instrument: "SPY".into(),
                epic: "IX.D.SPTRD.DAILY.IP".into(),
                direction: OrderSide::Sell,
                size: 10.0,
                order_type: crate::execution::traits::OrderType::Market,
                currency_code: "GBP".into(),
                expiry: "DFB".into(),
            }])
            .await
            .unwrap();

        assert!(engine.get_positions().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn partial_close_updates_size() {
        let engine = PaperExecutionEngine::new();

        engine
            .place_orders(vec![OrderRequest {
                instrument: "SPY".into(),
                epic: "IX.D.SPTRD.DAILY.IP".into(),
                direction: OrderSide::Buy,
                size: 10.0,
                order_type: crate::execution::traits::OrderType::Market,
                currency_code: "GBP".into(),
                expiry: "DFB".into(),
            }])
            .await
            .unwrap();

        engine
            .place_orders(vec![OrderRequest {
                instrument: "SPY".into(),
                epic: "IX.D.SPTRD.DAILY.IP".into(),
                direction: OrderSide::Sell,
                size: 3.0,
                order_type: crate::execution::traits::OrderType::Market,
                currency_code: "GBP".into(),
                expiry: "DFB".into(),
            }])
            .await
            .unwrap();

        let positions = engine.get_positions().await.unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].size, 7.0);
        assert!(matches!(positions[0].direction, OrderSide::Buy));
    }
}
