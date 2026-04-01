use std::collections::HashMap;

use crate::config::IgConfig;
use crate::execution::router::SizedOrder;
use crate::execution::traits::{OrderRequest, OrderType};

/// Bidirectional mapping between quantbot symbols and IG epics.
pub struct SymbolMapper {
    symbol_to_epic: HashMap<String, String>,
    epic_to_symbol: HashMap<String, String>,
    currency_codes: HashMap<String, String>,
    expiries: HashMap<String, String>,
}

impl SymbolMapper {
    pub fn from_config(config: &IgConfig) -> Self {
        let mut symbol_to_epic = HashMap::new();
        let mut epic_to_symbol = HashMap::new();
        let mut currency_codes = HashMap::new();
        let mut expiries = HashMap::new();

        for (symbol, inst) in &config.instruments {
            symbol_to_epic.insert(symbol.clone(), inst.epic.clone());
            epic_to_symbol.insert(inst.epic.clone(), symbol.clone());
            currency_codes.insert(symbol.clone(), inst.currency().to_string());
            expiries.insert(symbol.clone(), inst.expiry().to_string());
        }

        Self {
            symbol_to_epic,
            epic_to_symbol,
            currency_codes,
            expiries,
        }
    }

    pub fn quantbot_to_epic(&self, symbol: &str) -> Option<&str> {
        self.symbol_to_epic.get(symbol).map(|s| s.as_str())
    }

    pub fn epic_to_quantbot(&self, epic: &str) -> Option<&str> {
        self.epic_to_symbol.get(epic).map(|s| s.as_str())
    }

    /// Convert a SizedOrder from the backtest engine into an OrderRequest for the execution engine.
    pub fn order_request_from_sized_order(&self, order: &SizedOrder) -> Option<OrderRequest> {
        let epic = self.quantbot_to_epic(&order.instrument)?;
        let currency = self
            .currency_codes
            .get(&order.instrument)
            .cloned()
            .unwrap_or_else(|| "GBP".to_string());
        let expiry = self
            .expiries
            .get(&order.instrument)
            .cloned()
            .unwrap_or_else(|| "DFB".to_string());

        Some(OrderRequest {
            instrument: order.instrument.clone(),
            epic: epic.to_string(),
            direction: order.side,
            size: order.quantity,
            order_type: OrderType::Market,
            currency_code: currency,
            expiry,
        })
    }

    pub fn symbols(&self) -> impl Iterator<Item = &str> {
        self.symbol_to_epic.keys().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IgConfig, IgEnvironment, InstrumentConfig};
    use crate::core::portfolio::OrderSide;

    fn test_config() -> IgConfig {
        let mut instruments = HashMap::new();
        instruments.insert(
            "SPY".to_string(),
            InstrumentConfig {
                epic: "IX.D.SPTRD.DAILY.IP".to_string(),
                min_size: 0.1,
                size_step: 0.1,
                currency_code: None,
                expiry: None,
            },
        );
        instruments.insert(
            "GC=F".to_string(),
            InstrumentConfig {
                epic: "CC.D.GC.USS.IP".to_string(),
                min_size: 1.0,
                size_step: 1.0,
                currency_code: Some("USD".to_string()),
                expiry: Some("MAR-26".to_string()),
            },
        );
        IgConfig {
            environment: IgEnvironment::Demo,
            account_id: "TEST".to_string(),
            instruments,
        }
    }

    #[test]
    fn symbol_to_epic_lookup() {
        let mapper = SymbolMapper::from_config(&test_config());
        assert_eq!(
            mapper.quantbot_to_epic("SPY"),
            Some("IX.D.SPTRD.DAILY.IP")
        );
        assert_eq!(mapper.quantbot_to_epic("GC=F"), Some("CC.D.GC.USS.IP"));
        assert_eq!(mapper.quantbot_to_epic("UNKNOWN"), None);
    }

    #[test]
    fn epic_to_symbol_lookup() {
        let mapper = SymbolMapper::from_config(&test_config());
        assert_eq!(
            mapper.epic_to_quantbot("IX.D.SPTRD.DAILY.IP"),
            Some("SPY")
        );
        assert_eq!(mapper.epic_to_quantbot("CC.D.GC.USS.IP"), Some("GC=F"));
        assert_eq!(mapper.epic_to_quantbot("UNKNOWN.EPIC"), None);
    }

    #[test]
    fn order_request_from_sized_order_default_currency() {
        let mapper = SymbolMapper::from_config(&test_config());
        let sized = SizedOrder {
            instrument: "SPY".to_string(),
            side: OrderSide::Buy,
            quantity: 10.0,
            reference_price: 500.0,
            point_value: 1.0,
            notional: 5000.0,
            margin_required: 1000.0,
            spread_cost: 2.5,
        };

        let req = mapper.order_request_from_sized_order(&sized).unwrap();
        assert_eq!(req.instrument, "SPY");
        assert_eq!(req.epic, "IX.D.SPTRD.DAILY.IP");
        assert_eq!(req.size, 10.0);
        assert!(matches!(req.direction, OrderSide::Buy));
        assert_eq!(req.currency_code, "GBP");
        assert_eq!(req.expiry, "DFB");
    }

    #[test]
    fn order_request_custom_currency_and_expiry() {
        let mapper = SymbolMapper::from_config(&test_config());
        let sized = SizedOrder {
            instrument: "GC=F".to_string(),
            side: OrderSide::Sell,
            quantity: 2.0,
            reference_price: 2050.0,
            point_value: 100.0,
            notional: 410000.0,
            margin_required: 20500.0,
            spread_cost: 41.0,
        };

        let req = mapper.order_request_from_sized_order(&sized).unwrap();
        assert_eq!(req.currency_code, "USD");
        assert_eq!(req.expiry, "MAR-26");
    }

    #[test]
    fn order_request_unknown_symbol_returns_none() {
        let mapper = SymbolMapper::from_config(&test_config());
        let sized = SizedOrder {
            instrument: "AAPL".to_string(),
            side: OrderSide::Buy,
            quantity: 1.0,
            reference_price: 100.0,
            point_value: 1.0,
            notional: 100.0,
            margin_required: 20.0,
            spread_cost: 0.05,
        };
        assert!(mapper.order_request_from_sized_order(&sized).is_none());
    }

    #[test]
    fn bidirectional_consistency() {
        let mapper = SymbolMapper::from_config(&test_config());
        for sym in mapper.symbols() {
            let epic = mapper.quantbot_to_epic(sym).unwrap();
            let back = mapper.epic_to_quantbot(epic).unwrap();
            assert_eq!(sym, back);
        }
    }
}
