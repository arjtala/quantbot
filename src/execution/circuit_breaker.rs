use serde::{Deserialize, Serialize};

/// Simple circuit breaker that trips after consecutive failures.
/// When tripped, the caller should flatten and exit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreaker {
    pub max_consecutive_failures: u32,
    pub max_order_count: usize,
    pub max_single_order_size: Option<f64>,
    consecutive_failures: u32,
    tripped: bool,
}

impl CircuitBreaker {
    pub fn new(max_consecutive_failures: u32, max_order_count: usize) -> Self {
        Self {
            max_consecutive_failures,
            max_order_count,
            max_single_order_size: None,
            consecutive_failures: 0,
            tripped: false,
        }
    }

    pub fn with_max_order_size(mut self, max_size: f64) -> Self {
        self.max_single_order_size = Some(max_size);
        self
    }

    pub fn is_tripped(&self) -> bool {
        self.tripped
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= self.max_consecutive_failures {
            self.tripped = true;
        }
    }

    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.tripped = false;
    }

    /// Pre-trade check: validates order count and sizes BEFORE placing any orders.
    /// Returns Err with reason if the check fails.
    pub fn check_orders(&self, order_count: usize, max_order_size: f64) -> Result<(), String> {
        if self.tripped {
            return Err(format!(
                "circuit breaker tripped after {} consecutive failures",
                self.consecutive_failures
            ));
        }

        if order_count > self.max_order_count {
            return Err(format!(
                "order count {} exceeds max {}",
                order_count, self.max_order_count
            ));
        }

        if let Some(limit) = self.max_single_order_size {
            if max_order_size > limit {
                return Err(format!(
                    "max order size {:.1} exceeds limit {:.1}",
                    max_order_size, limit
                ));
            }
        }

        Ok(())
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(3, 12) // 3 failures, max 12 orders (2x 6-instrument universe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_trip_below_threshold() {
        let mut cb = CircuitBreaker::new(3, 10);
        cb.record_failure();
        cb.record_failure();
        assert!(!cb.is_tripped());
    }

    #[test]
    fn trips_at_threshold() {
        let mut cb = CircuitBreaker::new(3, 10);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_tripped());
    }

    #[test]
    fn success_resets_counter() {
        let mut cb = CircuitBreaker::new(3, 10);
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        cb.record_failure();
        cb.record_failure();
        assert!(!cb.is_tripped());
    }

    #[test]
    fn reset_clears_trip() {
        let mut cb = CircuitBreaker::new(3, 10);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_tripped());
        cb.reset();
        assert!(!cb.is_tripped());
    }

    #[test]
    fn check_orders_within_limits() {
        let cb = CircuitBreaker::new(3, 10);
        assert!(cb.check_orders(5, 100.0).is_ok());
    }

    #[test]
    fn check_orders_too_many() {
        let cb = CircuitBreaker::new(3, 10);
        assert!(cb.check_orders(11, 100.0).is_err());
    }

    #[test]
    fn check_orders_tripped() {
        let mut cb = CircuitBreaker::new(3, 10);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.check_orders(1, 1.0).is_err());
    }

    #[test]
    fn check_orders_size_limit() {
        let cb = CircuitBreaker::new(3, 10).with_max_order_size(50.0);
        assert!(cb.check_orders(1, 50.0).is_ok());
        assert!(cb.check_orders(1, 51.0).is_err());
    }

    #[test]
    fn default_values() {
        let cb = CircuitBreaker::default();
        assert_eq!(cb.max_consecutive_failures, 3);
        assert_eq!(cb.max_order_count, 12);
        assert!(!cb.is_tripped());
    }
}
