mod volatility;

use std::collections::HashMap;

use crate::agents::SignalAgent;
use crate::core::bar::BarSeries;
use crate::core::signal::{Signal, SignalDirection, SignalType};

use volatility::ewma_volatility;

/// Default lookback windows in trading days (~1m, 3m, 6m, 12m).
const DEFAULT_LOOKBACKS: &[usize] = &[21, 63, 126, 252];

/// Annualized target volatility (40%).
const VOL_TARGET: f64 = 0.40;

/// EWMA center of mass for volatility estimation.
const EWMA_COM: usize = 60;

pub struct TSMOMAgent {
    pub lookbacks: Vec<usize>,
    pub vol_target: f64,
    pub ewma_com: usize,
}

impl Default for TSMOMAgent {
    fn default() -> Self {
        Self {
            lookbacks: DEFAULT_LOOKBACKS.to_vec(),
            vol_target: VOL_TARGET,
            ewma_com: EWMA_COM,
        }
    }
}

impl TSMOMAgent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a TSMOM signal from bar data for a single instrument.
    pub fn generate_signal(&self, bars: &BarSeries, instrument: &str) -> Signal {
        let data = bars.bars();
        let max_lb = *self.lookbacks.iter().max().unwrap_or(&252);

        // Need at least max_lookback + 1 bars to compute trailing returns
        if data.len() < max_lb + 2 {
            return self.flat_signal(instrument, "insufficient_data", HashMap::new());
        }

        // Compute daily returns from close prices
        let closes: Vec<f64> = data.iter().map(|b| b.close).collect();
        let returns: Vec<f64> = closes.windows(2).map(|w| w[1] / w[0] - 1.0).collect();

        // Trailing returns and signs for each lookback
        let mut signs = Vec::with_capacity(self.lookbacks.len());
        let mut metadata = HashMap::new();
        let last_close = *closes.last().unwrap();

        for &lb in &self.lookbacks {
            let past_close = closes[closes.len() - 1 - lb];
            let ret = last_close / past_close - 1.0;
            metadata.insert(format!("ret_{lb}d"), ret);
            signs.push(ret.signum());
        }

        // Average sign across lookbacks → strength in [-1, 1]
        let avg_sign: f64 = signs.iter().sum::<f64>() / signs.len() as f64;

        // EWMA volatility — compute early so flat signals still carry vol_scalar
        let ann_vol = ewma_volatility(&returns, self.ewma_com, 20);
        let current_vol = *ann_vol.last().unwrap_or(&0.0);

        if current_vol > 1e-8 {
            let vol_scalar = self.vol_target / current_vol;
            metadata.insert("ann_vol".into(), current_vol);
            metadata.insert("vol_scalar".into(), vol_scalar);
        }

        if avg_sign == 0.0 {
            return self.flat_signal(instrument, "conflicting_signals", metadata);
        }

        // Confidence = fraction of lookbacks agreeing with majority
        let majority = avg_sign.signum();
        let agreement =
            signs.iter().filter(|&&s| s == majority).count() as f64 / signs.len() as f64;

        if current_vol < 1e-8 {
            return self.flat_signal(instrument, "zero_volatility", metadata);
        }

        let direction = if avg_sign > 0.0 {
            SignalDirection::Long
        } else {
            SignalDirection::Short
        };
        let strength = avg_sign.clamp(-1.0, 1.0);

        let mut sig = Signal::new(
            instrument.into(),
            direction,
            strength,
            agreement,
            "TSMOM".into(),
            SignalType::Quant,
        )
        .expect("TSMOM signal values are always valid");
        sig.metadata = metadata;
        sig
    }

    /// Compute the vol-targeted portfolio weight for a signal.
    pub fn compute_target_weight(signal: &Signal) -> f64 {
        let vol_scalar = signal.metadata.get("vol_scalar").copied().unwrap_or(1.0);
        signal.strength * signal.confidence * vol_scalar
    }

    fn flat_signal(
        &self,
        instrument: &str,
        reason: &str,
        mut metadata: HashMap<String, f64>,
    ) -> Signal {
        // Encode reason as a numeric code for HashMap<String, f64>
        let reason_code = match reason {
            "insufficient_data" => 1.0,
            "conflicting_signals" => 2.0,
            "zero_volatility" => 3.0,
            _ => 0.0,
        };
        metadata.insert("flat_reason".into(), reason_code);

        let mut sig = Signal::new(
            instrument.into(),
            SignalDirection::Flat,
            0.0,
            0.0,
            "TSMOM".into(),
            SignalType::Quant,
        )
        .expect("flat signal values are always valid");
        sig.metadata = metadata;
        sig
    }
}

impl SignalAgent for TSMOMAgent {
    fn name(&self) -> &str {
        "tsmom"
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Quant
    }

    fn generate_signal(&self, bars: &BarSeries, instrument: &str) -> Signal {
        self.generate_signal(bars, instrument)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bar::Bar;
    use chrono::NaiveDate;

    /// Generate a synthetic price series with a known trend.
    fn trending_bars(n: usize, start_price: f64, daily_return: f64) -> BarSeries {
        let mut bars = Vec::with_capacity(n);
        let mut price = start_price;
        let base_date = NaiveDate::from_ymd_opt(2023, 1, 2).unwrap();

        for i in 0..n {
            bars.push(Bar {
                date: base_date + chrono::Days::new(i as u64),
                open: price,
                high: price * 1.01,
                low: price * 0.99,
                close: price,
                volume: 10000.0,
            });
            price *= 1.0 + daily_return;
        }

        BarSeries::new(bars).unwrap()
    }

    #[test]
    fn strong_uptrend_gives_long() {
        // 300 days of +0.1% daily return → clear uptrend
        let bars = trending_bars(300, 100.0, 0.001);
        let agent = TSMOMAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        assert_eq!(sig.direction, SignalDirection::Long);
        assert!(sig.strength > 0.0);
        assert!(sig.confidence > 0.5);
        assert!(sig.metadata.contains_key("ann_vol"));
        assert!(sig.metadata.contains_key("vol_scalar"));
    }

    #[test]
    fn strong_downtrend_gives_short() {
        // 300 days of -0.1% daily return → clear downtrend
        let bars = trending_bars(300, 100.0, -0.001);
        let agent = TSMOMAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        assert_eq!(sig.direction, SignalDirection::Short);
        assert!(sig.strength < 0.0);
        assert!(sig.confidence > 0.5);
    }

    #[test]
    fn insufficient_data_gives_flat() {
        let bars = trending_bars(50, 100.0, 0.001); // < 253 bars needed
        let agent = TSMOMAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        assert_eq!(sig.direction, SignalDirection::Flat);
        assert!((sig.strength - 0.0).abs() < 1e-10);
    }

    #[test]
    fn vol_scalar_targets_40pct() {
        let bars = trending_bars(300, 100.0, 0.001);
        let agent = TSMOMAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        let vol = sig.metadata["ann_vol"];
        let scalar = sig.metadata["vol_scalar"];
        assert!((scalar - 0.40 / vol).abs() < 1e-10);
    }

    #[test]
    fn target_weight_combines_signal_and_vol() {
        let bars = trending_bars(300, 100.0, 0.001);
        let agent = TSMOMAgent::new();
        let sig = agent.generate_signal(&bars, "TEST");

        let weight = TSMOMAgent::compute_target_weight(&sig);
        let expected = sig.strength * sig.confidence * sig.metadata["vol_scalar"];
        assert!((weight - expected).abs() < 1e-10);
    }
}
