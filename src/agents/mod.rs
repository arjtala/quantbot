#[cfg(feature = "track-b")]
pub mod combiner;
#[cfg(feature = "track-b")]
pub mod indicator;
pub mod risk;
pub mod tsmom;

use crate::core::bar::BarSeries;
use crate::core::signal::{Signal, SignalType};
use crate::db::LlmCacheEntry;

/// Trait for signal-generating agents.
///
/// All methods are object-safe so `Box<dyn SignalAgent>` works.
pub trait SignalAgent {
    fn name(&self) -> &str;
    fn signal_type(&self) -> SignalType;
    fn generate_signal(&self, bars: &BarSeries, instrument: &str) -> Signal;

    /// Drain any pending LLM cache entries collected during signal generation.
    /// Default: returns empty vec (non-LLM agents have nothing to cache).
    fn take_cache_entries(&self) -> Vec<LlmCacheEntry> {
        vec![]
    }
}
