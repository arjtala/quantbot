use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetClass {
    Crypto,
    Equity,
    Futures,
    Fx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    pub symbol: String,
    pub name: String,
    pub asset_class: AssetClass,
    pub point_value: f64,
}

impl Instrument {
    pub fn new(symbol: &str, name: &str, asset_class: AssetClass) -> Self {
        Self {
            symbol: symbol.into(),
            name: name.into(),
            asset_class,
            point_value: 1.0,
        }
    }

    pub fn with_point_value(mut self, pv: f64) -> Self {
        self.point_value = pv;
        self
    }
}

/// The 6 focused instruments from Phase 2 validation (Sharpe 1.112 after costs).
pub static TRADEABLE_UNIVERSE: LazyLock<Vec<Instrument>> = LazyLock::new(|| {
    vec![
        Instrument::new("GLD", "Gold ETF", AssetClass::Equity),
        Instrument::new("GC=F", "Gold Futures", AssetClass::Futures)
            .with_point_value(100.0),
        Instrument::new("SPY", "S&P 500 ETF", AssetClass::Equity),
        Instrument::new("GBPUSD=X", "GBP/USD", AssetClass::Fx),
        Instrument::new("USDCHF=X", "USD/CHF", AssetClass::Fx),
        Instrument::new("USDJPY=X", "USD/JPY", AssetClass::Fx),
    ]
});

/// Look up an instrument by symbol in the tradeable universe.
/// Falls back to a default equity instrument if not found.
pub fn get_instrument(symbol: &str) -> Instrument {
    TRADEABLE_UNIVERSE
        .iter()
        .find(|i| i.symbol == symbol)
        .cloned()
        .unwrap_or_else(|| Instrument::new(symbol, symbol, AssetClass::Equity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tradeable_universe_has_six_instruments() {
        assert_eq!(TRADEABLE_UNIVERSE.len(), 6);
    }

    #[test]
    fn lookup_known_instrument() {
        let gc = get_instrument("GC=F");
        assert_eq!(gc.symbol, "GC=F");
        assert_eq!(gc.asset_class, AssetClass::Futures);
        assert!((gc.point_value - 100.0).abs() < 1e-10);
    }

    #[test]
    fn lookup_unknown_falls_back() {
        let unknown = get_instrument("AAPL");
        assert_eq!(unknown.symbol, "AAPL");
        assert_eq!(unknown.asset_class, AssetClass::Equity);
        assert!((unknown.point_value - 1.0).abs() < 1e-10);
    }
}
