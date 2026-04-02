use std::collections::HashMap;
use std::sync::Mutex;

use crate::audit::TargetEntry;
use crate::core::signal::SignalDirection;
use crate::db::Db;
use crate::execution::traits::{
    DealStatus, OrderAck, OrderRequest,
};

// ─── Recorder ───────────────────────────────────────────────────

/// Records trading activity to SQLite alongside live execution.
///
/// This is **not** a decorator on `ExecutionEngine` (which would require
/// duplicating the RPITIT trait). Instead it wraps a `Db` and provides
/// typed `record_*` methods that the live command calls at each stage.
///
/// Usage in `cmd_live` / `run_rebalance`:
/// ```ignore
/// let recorder = Recorder::new(db, run_id);
/// recorder.record_run_start(...);
/// recorder.record_signals(...);
/// // ... engine.place_orders() ...
/// recorder.record_orders_submitted(...);
/// recorder.record_orders_confirmed(...);
/// recorder.record_run_end(...);
/// ```
pub struct Recorder {
    db: Mutex<Db>,
    run_id: String,
}

impl Recorder {
    /// Create a new recorder. Inserts the run row immediately.
    pub fn new(db: Db, run_id: &str, config_json: &str, nav_usd: f64) -> Self {
        if let Err(e) = db.insert_run(run_id, config_json, nav_usd) {
            eprintln!("  WARN: SQLite insert_run failed: {e}");
        }
        Self {
            db: Mutex::new(db),
            run_id: run_id.to_string(),
        }
    }

    /// Record target signals from the signal generation phase.
    pub fn record_signals(&self, targets: &[TargetEntry], signals: &HashMap<String, (SignalDirection, f64, f64)>) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                return;
            }
        };
        for target in targets {
            let (direction, strength, confidence) = signals
                .get(&target.instrument)
                .copied()
                .unwrap_or((SignalDirection::Flat, 0.0, 0.0));
            let dir_str = match direction {
                SignalDirection::Long => "Long",
                SignalDirection::Short => "Short",
                SignalDirection::Flat => "Flat",
            };
            if let Err(e) = db.insert_signal(
                &self.run_id,
                &target.instrument,
                dir_str,
                strength,
                confidence,
                target.weight,
            ) {
                eprintln!("  WARN: SQLite insert_signal failed: {e}");
            }
        }
    }

    /// Record target positions (what we want to hold).
    pub fn record_target_positions(&self, targets: &[TargetEntry]) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                return;
            }
        };
        for t in targets {
            if let Err(e) = db.insert_position(
                &self.run_id,
                &t.instrument,
                t.signed_deal_size,
                "target",
            ) {
                eprintln!("  WARN: SQLite insert_position failed: {e}");
            }
        }
    }

    /// Record actual positions fetched from the broker.
    pub fn record_actual_positions(&self, positions: &HashMap<String, f64>) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                return;
            }
        };
        for (instrument, &size) in positions {
            if let Err(e) = db.insert_position(
                &self.run_id,
                instrument,
                size,
                "actual",
            ) {
                eprintln!("  WARN: SQLite insert_position failed: {e}");
            }
        }
    }

    /// Record orders that were submitted to the broker.
    pub fn record_orders_submitted(&self, orders: &[OrderRequest]) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                return;
            }
        };
        for o in orders {
            let dir_str = match o.direction {
                crate::core::portfolio::OrderSide::Buy => "BUY",
                crate::core::portfolio::OrderSide::Sell => "SELL",
            };
            if let Err(e) = db.insert_order(
                &self.run_id,
                &o.instrument,
                &o.epic,
                dir_str,
                o.size,
                None,
                None,
            ) {
                eprintln!("  WARN: SQLite insert_order failed: {e}");
            }
        }
    }

    /// Record order acknowledgements (fills/rejections).
    pub fn record_orders_confirmed(&self, acks: &[OrderAck]) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                return;
            }
        };
        for ack in acks {
            let status_str = match ack.status {
                DealStatus::Accepted => "Accepted",
                DealStatus::Rejected => "Rejected",
                DealStatus::Pending => "Pending",
            };
            // Update existing order row by deal_reference, or insert a new one
            // For simplicity, insert a confirmation row — the orders table has both
            // submission and confirmation entries, distinguishable by status being set
            if let Err(e) = db.insert_order(
                &self.run_id,
                &ack.instrument,
                "", // epic not in ack
                "",  // direction not in ack
                0.0, // size not in ack
                Some(&ack.deal_reference),
                Some(status_str),
            ) {
                eprintln!("  WARN: SQLite insert_order (confirm) failed: {e}");
            }
        }
    }

    /// Record post-trade positions (after execution, for verification).
    pub fn record_post_trade_positions(&self, positions: &HashMap<String, f64>) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                return;
            }
        };
        for (instrument, &size) in positions {
            if let Err(e) = db.insert_position(
                &self.run_id,
                instrument,
                size,
                "post_trade",
            ) {
                eprintln!("  WARN: SQLite insert_position failed: {e}");
            }
        }
    }

    /// Finalize the run with outcome and duration.
    pub fn record_run_end(&self, outcome: &str, duration_ms: u64) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                return;
            }
        };
        if let Err(e) = db.finish_run(&self.run_id, outcome, duration_ms) {
            eprintln!("  WARN: SQLite finish_run failed: {e}");
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::portfolio::OrderSide;

    fn make_recorder() -> Recorder {
        let db = Db::open_memory().unwrap();
        Recorder::new(db, "test-run", "{\"engine\":\"paper\"}", 1_000_000.0)
    }

    #[test]
    fn record_full_lifecycle() {
        let rec = make_recorder();

        // Record signals
        let targets = vec![
            TargetEntry {
                instrument: "SPY".into(),
                signed_deal_size: -282.0,
                weight: -0.16,
            },
            TargetEntry {
                instrument: "GLD".into(),
                signed_deal_size: 1388.0,
                weight: 0.40,
            },
        ];
        let mut signals = HashMap::new();
        signals.insert("SPY".to_string(), (SignalDirection::Short, -0.5, 0.75));
        signals.insert("GLD".to_string(), (SignalDirection::Long, 1.0, 1.0));
        rec.record_signals(&targets, &signals);

        // Record target positions
        rec.record_target_positions(&targets);

        // Record actual positions (empty — no existing positions)
        rec.record_actual_positions(&HashMap::new());

        // Record submitted orders
        let orders = vec![OrderRequest {
            instrument: "SPY".into(),
            epic: "IX.D.SPTRD.DAILY.IP".into(),
            direction: OrderSide::Sell,
            size: 282.0,
            order_type: crate::execution::traits::OrderType::Market,
            currency_code: "GBP".into(),
            expiry: "DFB".into(),
        }];
        rec.record_orders_submitted(&orders);

        // Record confirmations
        let acks = vec![OrderAck {
            deal_reference: "REF1".into(),
            instrument: "SPY".into(),
            status: DealStatus::Accepted,
        }];
        rec.record_orders_confirmed(&acks);

        // Finalize
        rec.record_run_end("SUCCESS", 4500);

        // Verify via Db queries
        let db = rec.db.lock().unwrap();
        let runs = db.list_runs(10).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].outcome.as_deref(), Some("SUCCESS"));

        let sigs = db.signals_for_run("test-run").unwrap();
        assert_eq!(sigs.len(), 2);

        let ords = db.orders_for_run("test-run").unwrap();
        assert_eq!(ords.len(), 2); // 1 submission + 1 confirmation
    }

    #[test]
    fn record_dry_run() {
        let rec = make_recorder();
        rec.record_run_end("DRY_RUN", 500);

        let db = rec.db.lock().unwrap();
        let runs = db.list_runs(1).unwrap();
        assert_eq!(runs[0].outcome.as_deref(), Some("DRY_RUN"));
    }
}
