pub mod ta;

pub mod llm_client;
pub mod parser;
pub mod llm_agent;
pub mod prompt_loader;

use std::collections::HashMap;

use crate::agents::SignalAgent;
use crate::core::bar::BarSeries;
use crate::core::signal::{Signal, SignalDirection, SignalType};
use ta::compute_rsi;

/// RSI period for the dummy indicator.
const RSI_PERIOD: usize = 14;

/// RSI below this threshold → Long (oversold).
const RSI_OVERSOLD: f64 = 30.0;

/// RSI above this threshold → Short (overbought).
const RSI_OVERBOUGHT: f64 = 70.0;

/// Fixed confidence for non-flat signals.
const SIGNAL_CONFIDENCE: f64 = 0.7;

/// Dummy indicator agent using 14-period RSI (Wilder's smoothing).
///
/// RSI < 30 → Long (oversold), RSI > 70 → Short (overbought), else Flat.
/// Signals are advisory only in B1 — they don't drive sizing.
pub struct DummyIndicatorAgent;

impl DummyIndicatorAgent {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DummyIndicatorAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalAgent for DummyIndicatorAgent {
    fn name(&self) -> &str {
        "indicator"
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Quant
    }

    fn generate_signal(&self, bars: &BarSeries, instrument: &str) -> Signal {
        let data = bars.bars();

        // Need at least RSI_PERIOD + 1 bars to compute RSI
        if data.len() < RSI_PERIOD + 1 {
            return flat_signal(instrument, 0.0);
        }

        let closes: Vec<f64> = data.iter().map(|b| b.close).collect();
        let rsi = compute_rsi(&closes, RSI_PERIOD);

        let (direction, strength) = if rsi < RSI_OVERSOLD {
            // Oversold → Long. Strength = distance from threshold, scaled to [0, 1]
            let s = (RSI_OVERSOLD - rsi) / RSI_OVERSOLD;
            (SignalDirection::Long, s.clamp(0.0, 1.0))
        } else if rsi > RSI_OVERBOUGHT {
            // Overbought → Short. Strength negative, magnitude = distance from threshold
            let s = (rsi - RSI_OVERBOUGHT) / (100.0 - RSI_OVERBOUGHT);
            (SignalDirection::Short, -(s.clamp(0.0, 1.0)))
        } else {
            return flat_signal(instrument, rsi);
        };

        let mut metadata = HashMap::new();
        metadata.insert("rsi".into(), rsi);

        let mut sig = Signal::new(
            instrument.into(),
            direction,
            strength,
            SIGNAL_CONFIDENCE,
            "indicator".into(),
            SignalType::Quant,
        )
        .expect("indicator signal values are always valid");
        sig.metadata = metadata;
        sig
    }
}

fn flat_signal(instrument: &str, rsi: f64) -> Signal {
    let mut metadata = HashMap::new();
    metadata.insert("rsi".into(), rsi);

    let mut sig = Signal::new(
        instrument.into(),
        SignalDirection::Flat,
        0.0,
        0.0,
        "indicator".into(),
        SignalType::Quant,
    )
    .expect("flat signal values are always valid");
    sig.metadata = metadata;
    sig
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bar::Bar;
    use chrono::NaiveDate;

    fn make_bars(prices: &[f64]) -> BarSeries {
        let base_date = NaiveDate::from_ymd_opt(2023, 1, 2).unwrap();
        let bars: Vec<Bar> = prices
            .iter()
            .enumerate()
            .map(|(i, &price)| Bar {
                date: base_date + chrono::Days::new(i as u64),
                open: price,
                high: price * 1.01,
                low: price * 0.99,
                close: price,
                volume: 10000.0,
            })
            .collect();
        BarSeries::new(bars).unwrap()
    }

    /// Steadily declining prices → low RSI → Long (oversold)
    #[test]
    fn downtrend_gives_long() {
        let mut prices = vec![100.0];
        for i in 1..30 {
            prices.push(100.0 - i as f64 * 1.5);
        }
        let bars = make_bars(&prices);
        let agent = DummyIndicatorAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        assert_eq!(sig.direction, SignalDirection::Long);
        assert!(sig.strength > 0.0);
        assert_eq!(sig.confidence, SIGNAL_CONFIDENCE);
    }

    /// Steadily rising prices → high RSI → Short (overbought)
    #[test]
    fn uptrend_gives_short() {
        let mut prices = vec![100.0];
        for i in 1..30 {
            prices.push(100.0 + i as f64 * 1.5);
        }
        let bars = make_bars(&prices);
        let agent = DummyIndicatorAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        assert_eq!(sig.direction, SignalDirection::Short);
        assert!(sig.strength < 0.0);
        assert_eq!(sig.confidence, SIGNAL_CONFIDENCE);
    }

    /// Sideways (flat prices) → RSI near 50 → Flat
    #[test]
    fn sideways_gives_flat() {
        // Alternating up/down around 100
        let mut prices = Vec::new();
        for i in 0..30 {
            prices.push(100.0 + if i % 2 == 0 { 0.5 } else { -0.5 });
        }
        let bars = make_bars(&prices);
        let agent = DummyIndicatorAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        assert_eq!(sig.direction, SignalDirection::Flat);
        assert!((sig.strength - 0.0).abs() < 1e-10);
    }

    /// Insufficient data → Flat
    #[test]
    fn insufficient_data_gives_flat() {
        let bars = make_bars(&[100.0, 101.0, 102.0]);
        let agent = DummyIndicatorAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        assert_eq!(sig.direction, SignalDirection::Flat);
    }

    /// RSI value stored in metadata
    #[test]
    fn metadata_contains_rsi() {
        let mut prices = vec![100.0];
        for i in 1..30 {
            prices.push(100.0 - i as f64 * 1.5);
        }
        let bars = make_bars(&prices);
        let agent = DummyIndicatorAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        assert!(sig.metadata.contains_key("rsi"));
        let rsi = sig.metadata["rsi"];
        assert!(rsi >= 0.0 && rsi <= 100.0);
    }

    /// Agent name is "indicator"
    #[test]
    fn agent_name() {
        let agent = DummyIndicatorAgent::new();
        assert_eq!(agent.name(), "indicator");
    }

    /// Signal type is Quant
    #[test]
    fn agent_signal_type() {
        let agent = DummyIndicatorAgent::new();
        assert_eq!(agent.signal_type(), SignalType::Quant);
    }
}
