use tokio::sync::Mutex;

use crate::config::IgConfig;
use crate::core::portfolio::OrderSide;
use crate::execution::ig::client::IgClient;
use crate::execution::ig::errors::IgError;
use crate::execution::ig::mapping::SymbolMapper;
use crate::execution::traits::{
    DealStatus, ExecutionEngine, ExecutionError, LivePosition, OrderAck, OrderRequest, OrderStatus,
};

pub struct IgExecutionEngine {
    client: Mutex<IgClient>,
    mapper: SymbolMapper,
}

impl IgExecutionEngine {
    pub fn new(config: &IgConfig) -> Result<Self, IgError> {
        let client = IgClient::new(config)?;
        let mapper = SymbolMapper::from_config(config);
        Ok(Self {
            client: Mutex::new(client),
            mapper,
        })
    }
}

impl ExecutionEngine for IgExecutionEngine {
    async fn health_check(&self) -> Result<(), ExecutionError> {
        let mut client = self.client.lock().await;
        client
            .ensure_authenticated()
            .await
            .map_err(|e| ExecutionError::AuthFailed(e.to_string()))?;
        client
            .get_positions()
            .await
            .map_err(|e| ExecutionError::Other(e.into()))?;
        Ok(())
    }

    async fn get_positions(&self) -> Result<Vec<LivePosition>, ExecutionError> {
        let mut client = self.client.lock().await;
        let resp = client
            .get_positions()
            .await
            .map_err(|e| ExecutionError::Other(e.into()))?;

        let mut positions = Vec::new();
        for ig_pos in resp.positions {
            let epic = &ig_pos.market.epic;
            let instrument = match self.mapper.epic_to_quantbot(epic) {
                Some(sym) => sym.to_string(),
                None => {
                    eprintln!("  IG: unknown epic {}, skipping", epic);
                    continue;
                }
            };

            let direction = match ig_pos.position.direction.as_str() {
                "BUY" => OrderSide::Buy,
                "SELL" => OrderSide::Sell,
                other => {
                    eprintln!("  IG: unknown direction '{}' for {}, skipping", other, epic);
                    continue;
                }
            };

            positions.push(LivePosition {
                deal_id: ig_pos.position.deal_id,
                instrument,
                epic: epic.to_string(),
                direction,
                size: ig_pos.position.size,
                open_level: ig_pos.position.level,
                currency: ig_pos.position.currency,
            });
        }

        Ok(positions)
    }

    async fn place_orders(
        &self,
        orders: Vec<OrderRequest>,
    ) -> Result<Vec<OrderAck>, ExecutionError> {
        let mut client = self.client.lock().await;
        let mut acks = Vec::with_capacity(orders.len());

        for order in &orders {
            let ig_direction = match order.direction {
                OrderSide::Buy => "BUY",
                OrderSide::Sell => "SELL",
            };

            let create_req = crate::execution::ig::types::CreatePositionRequest {
                epic: order.epic.clone(),
                direction: ig_direction.to_string(),
                size: order.size,
                order_type: "MARKET".to_string(),
                currency_code: order.currency_code.clone(),
                expiry: order.expiry.clone(),
                force_open: false,
                guaranteed_stop: false,
            };

            eprintln!(
                "  IG: placing {} {} size={} epic={}",
                ig_direction, order.instrument, order.size, order.epic
            );

            let deal_ref = client
                .create_position(&create_req)
                .await
                .map_err(|e| ExecutionError::Other(e.into()))?;

            // Wait briefly then confirm
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let confirmation = client
                .get_deal_confirmation(&deal_ref.deal_reference)
                .await
                .map_err(|e| ExecutionError::Other(e.into()))?;

            let status = match confirmation.deal_status.as_str() {
                "ACCEPTED" => DealStatus::Accepted,
                "REJECTED" => {
                    eprintln!(
                        "  IG: order REJECTED for {} — {}",
                        order.instrument, confirmation.reason
                    );
                    DealStatus::Rejected
                }
                _ => DealStatus::Pending,
            };

            if status == DealStatus::Rejected {
                return Err(ExecutionError::OrderRejected {
                    reason: confirmation.reason,
                    deal_reference: deal_ref.deal_reference,
                });
            }

            eprintln!(
                "  IG: {} confirmed — deal_id={}, level={:?}, size={:?}",
                order.instrument,
                confirmation.deal_id,
                confirmation.level,
                confirmation.size
            );

            acks.push(OrderAck {
                deal_reference: deal_ref.deal_reference,
                instrument: order.instrument.clone(),
                status,
            });
        }

        Ok(acks)
    }

    async fn get_order_status(
        &self,
        deal_refs: &[String],
    ) -> Result<Vec<OrderStatus>, ExecutionError> {
        let mut client = self.client.lock().await;
        let mut statuses = Vec::with_capacity(deal_refs.len());

        for deal_ref in deal_refs {
            let confirmation = client
                .get_deal_confirmation(deal_ref)
                .await
                .map_err(|e| ExecutionError::Other(e.into()))?;

            let status = match confirmation.deal_status.as_str() {
                "ACCEPTED" => DealStatus::Accepted,
                "REJECTED" => DealStatus::Rejected,
                _ => DealStatus::Pending,
            };

            statuses.push(OrderStatus {
                deal_reference: deal_ref.clone(),
                deal_id: Some(confirmation.deal_id),
                status,
                reason: Some(confirmation.reason),
                level: confirmation.level,
                size: confirmation.size,
            });
        }

        Ok(statuses)
    }

    async fn flatten_all(&self) -> Result<(), ExecutionError> {
        // Get positions — release lock between steps
        let positions = {
            let mut client = self.client.lock().await;
            let resp = client
                .get_positions()
                .await
                .map_err(|e| ExecutionError::Other(e.into()))?;

            let mut positions = Vec::new();
            for ig_pos in resp.positions {
                let epic = &ig_pos.market.epic;
                let instrument = match self.mapper.epic_to_quantbot(epic) {
                    Some(sym) => sym.to_string(),
                    None => {
                        eprintln!("  IG: unknown epic {}, skipping", epic);
                        continue;
                    }
                };

                let direction = match ig_pos.position.direction.as_str() {
                    "BUY" => OrderSide::Buy,
                    "SELL" => OrderSide::Sell,
                    other => {
                        eprintln!("  IG: unknown direction '{}' for {}, skipping", other, epic);
                        continue;
                    }
                };

                positions.push(LivePosition {
                    deal_id: ig_pos.position.deal_id,
                    instrument,
                    epic: epic.to_string(),
                    direction,
                    size: ig_pos.position.size,
                    open_level: ig_pos.position.level,
                    currency: ig_pos.position.currency,
                });
            }
            positions
        };

        if positions.is_empty() {
            eprintln!("  IG: no positions to flatten");
            return Ok(());
        }

        let mut client = self.client.lock().await;

        for pos in &positions {
            let close_direction = match pos.direction {
                OrderSide::Buy => "SELL",
                OrderSide::Sell => "BUY",
            };

            let close_req = crate::execution::ig::types::ClosePositionRequest {
                deal_id: pos.deal_id.clone(),
                direction: close_direction.to_string(),
                size: pos.size,
                order_type: "MARKET".to_string(),
            };

            eprintln!(
                "  IG: closing {} {} size={} (deal_id={})",
                pos.instrument, close_direction, pos.size, pos.deal_id
            );

            let deal_ref = client
                .close_position(&close_req)
                .await
                .map_err(|e| ExecutionError::Other(e.into()))?;

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let confirmation = client
                .get_deal_confirmation(&deal_ref.deal_reference)
                .await
                .map_err(|e| ExecutionError::Other(e.into()))?;

            if confirmation.deal_status != "ACCEPTED" {
                eprintln!(
                    "  IG: close REJECTED for {} — {}",
                    pos.instrument, confirmation.reason
                );
                return Err(ExecutionError::OrderRejected {
                    reason: confirmation.reason,
                    deal_reference: deal_ref.deal_reference,
                });
            }

            eprintln!("  IG: closed {} — deal_id={}", pos.instrument, confirmation.deal_id);
        }

        Ok(())
    }
}
