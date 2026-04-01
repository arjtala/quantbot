// IgExecutionEngine — skeleton for PR 1, completed in PR 2.

use crate::execution::traits::{
    ExecutionEngine, ExecutionError, LivePosition, OrderAck, OrderRequest, OrderStatus,
};

#[derive(Default)]
pub struct IgExecutionEngine {
    // Will hold IgClient + SymbolMapper in PR 2
}

impl ExecutionEngine for IgExecutionEngine {
    async fn health_check(&self) -> Result<(), ExecutionError> {
        todo!("PR 2: authenticate + get_positions")
    }

    async fn get_positions(&self) -> Result<Vec<LivePosition>, ExecutionError> {
        todo!("PR 2: fetch and map IG positions")
    }

    async fn place_orders(
        &self,
        _orders: Vec<OrderRequest>,
    ) -> Result<Vec<OrderAck>, ExecutionError> {
        todo!("PR 2: sequential order placement")
    }

    async fn get_order_status(
        &self,
        _deal_refs: &[String],
    ) -> Result<Vec<OrderStatus>, ExecutionError> {
        todo!("PR 2: fetch deal confirmations")
    }

    async fn flatten_all(&self) -> Result<(), ExecutionError> {
        todo!("PR 2: close all positions")
    }
}
