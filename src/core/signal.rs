use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalDirection {
    Long,
    Short,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalType {
    Quant,
    Llm,
    Combined,
}

#[derive(Debug, Error)]
pub enum SignalError {
    #[error("strength must be in [-1, 1], got {0}")]
    InvalidStrength(f64),
    #[error("confidence must be in [0, 1], got {0}")]
    InvalidConfidence(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub instrument: String,
    pub direction: SignalDirection,
    pub strength: f64,
    pub confidence: f64,
    pub agent_name: String,
    pub signal_type: SignalType,
    pub horizon_days: u32,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, f64>,
}

impl Signal {
    pub fn new(
        instrument: String,
        direction: SignalDirection,
        strength: f64,
        confidence: f64,
        agent_name: String,
        signal_type: SignalType,
    ) -> Result<Self, SignalError> {
        if !(-1.0..=1.0).contains(&strength) {
            return Err(SignalError::InvalidStrength(strength));
        }
        if !(0.0..=1.0).contains(&confidence) {
            return Err(SignalError::InvalidConfidence(confidence));
        }
        Ok(Self {
            instrument,
            direction,
            strength,
            confidence,
            agent_name,
            signal_type,
            horizon_days: 21,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        })
    }

    pub fn sized_strength(&self) -> f64 {
        self.strength * self.confidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_signal() {
        let sig = Signal::new(
            "SPY".into(),
            SignalDirection::Long,
            0.8,
            0.9,
            "tsmom".into(),
            SignalType::Quant,
        )
        .unwrap();
        assert!((sig.sized_strength() - 0.72).abs() < 1e-10);
        assert_eq!(sig.horizon_days, 21);
    }

    #[test]
    fn strength_out_of_range() {
        let err = Signal::new(
            "SPY".into(),
            SignalDirection::Long,
            1.5,
            0.5,
            "test".into(),
            SignalType::Quant,
        )
        .unwrap_err();
        assert!(matches!(err, SignalError::InvalidStrength(_)));
    }

    #[test]
    fn confidence_out_of_range() {
        let err = Signal::new(
            "SPY".into(),
            SignalDirection::Flat,
            0.0,
            -0.1,
            "test".into(),
            SignalType::Llm,
        )
        .unwrap_err();
        assert!(matches!(err, SignalError::InvalidConfidence(_)));
    }
}
