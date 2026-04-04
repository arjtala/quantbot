#[cfg(feature = "track-b")]
pub mod indicator;
pub mod risk;
pub mod tsmom;

use crate::core::bar::BarSeries;
use crate::core::signal::{Signal, SignalType};

/// Trait for signal-generating agents.
///
/// All methods are object-safe so `Box<dyn SignalAgent>` works.
pub trait SignalAgent {
    fn name(&self) -> &str;
    fn signal_type(&self) -> SignalType;
    fn generate_signal(&self, bars: &BarSeries, instrument: &str) -> Signal;
}
