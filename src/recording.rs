use std::collections::HashMap;
use std::sync::Mutex;

use crate::audit::TargetEntry;
use crate::core::signal::SignalDirection;
use crate::db::Db;
use crate::execution::traits::{
    DealStatus, OrderAck, OrderRequest,
};

// ─── Signal Record ──────────────────────────────────────────────

/// A signal record for multi-agent provenance recording.
pub struct SignalRecord {
    pub instrument: String,
    pub agent_name: String,
    pub direction: SignalDirection,
    pub strength: f64,
    pub confidence: f64,
    pub weight: f64,
}

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
    write_failed: Mutex<bool>,
}

impl Recorder {
    /// Create a new recorder. Inserts the run row immediately.
    pub fn new(db: Db, run_id: &str, config_json: &str, nav_usd: f64) -> Self {
        let mut failed = false;
        if let Err(e) = db.insert_run(run_id, config_json, nav_usd) {
            eprintln!("  WARN: SQLite insert_run failed: {e}");
            failed = true;
        }
        Self {
            db: Mutex::new(db),
            run_id: run_id.to_string(),
            write_failed: Mutex::new(failed),
        }
    }

    /// Whether any SQLite write has failed during this run.
    pub fn write_failed(&self) -> bool {
        *self.write_failed.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Mark a write failure.
    fn mark_failed(&self) {
        if let Ok(mut f) = self.write_failed.lock() {
            *f = true;
        }
    }

    /// Record target signals from the signal generation phase.
    pub fn record_signals(&self, records: &[SignalRecord]) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                self.mark_failed();
                return;
            }
        };
        let result = db.with_transaction(|conn| {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            for rec in records {
                let dir_str = match rec.direction {
                    SignalDirection::Long => "Long",
                    SignalDirection::Short => "Short",
                    SignalDirection::Flat => "Flat",
                };
                conn.execute(
                    "INSERT INTO signals (run_id, instrument, agent_name, direction, strength, confidence, weight, ts)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![&self.run_id, &rec.instrument, &rec.agent_name, dir_str, rec.strength, rec.confidence, rec.weight, &now],
                )?;
            }
            Ok(())
        });
        if let Err(e) = result {
            eprintln!("  WARN: SQLite batch insert_signal failed: {e}");
            self.mark_failed();
        }
    }

    /// Record target positions (what we want to hold).
    pub fn record_target_positions(&self, targets: &[TargetEntry]) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                self.mark_failed();
                return;
            }
        };
        let result = db.with_transaction(|conn| {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            for t in targets {
                conn.execute(
                    "INSERT INTO positions (run_id, instrument, signed_deal_size, source, ts)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![&self.run_id, &t.instrument, t.signed_deal_size, "target", &now],
                )?;
            }
            Ok(())
        });
        if let Err(e) = result {
            eprintln!("  WARN: SQLite batch insert_position failed: {e}");
            self.mark_failed();
        }
    }

    /// Record actual positions fetched from the broker.
    pub fn record_actual_positions(&self, positions: &HashMap<String, f64>) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                self.mark_failed();
                return;
            }
        };
        let result = db.with_transaction(|conn| {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            for (instrument, &size) in positions {
                conn.execute(
                    "INSERT INTO positions (run_id, instrument, signed_deal_size, source, ts)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![&self.run_id, instrument, size, "actual", &now],
                )?;
            }
            Ok(())
        });
        if let Err(e) = result {
            eprintln!("  WARN: SQLite batch insert_position (actual) failed: {e}");
            self.mark_failed();
        }
    }

    /// Record orders that were submitted to the broker.
    pub fn record_orders_submitted(&self, orders: &[OrderRequest]) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                self.mark_failed();
                return;
            }
        };
        let result = db.with_transaction(|conn| {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            for o in orders {
                let dir_str = match o.direction {
                    crate::core::portfolio::OrderSide::Buy => "BUY",
                    crate::core::portfolio::OrderSide::Sell => "SELL",
                };
                conn.execute(
                    "INSERT INTO orders (run_id, instrument, epic, direction, size, deal_reference, status, ts)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![&self.run_id, &o.instrument, &o.epic, dir_str, o.size, Option::<&str>::None, Option::<&str>::None, &now],
                )?;
            }
            Ok(())
        });
        if let Err(e) = result {
            eprintln!("  WARN: SQLite batch insert_order failed: {e}");
            self.mark_failed();
        }
    }

    /// Record order acknowledgements (fills/rejections).
    pub fn record_orders_confirmed(&self, acks: &[OrderAck]) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                self.mark_failed();
                return;
            }
        };
        let result = db.with_transaction(|conn| {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            for ack in acks {
                let status_str = match ack.status {
                    DealStatus::Accepted => "Accepted",
                    DealStatus::Rejected => "Rejected",
                    DealStatus::Pending => "Pending",
                };
                conn.execute(
                    "INSERT INTO orders (run_id, instrument, epic, direction, size, deal_reference, status, ts)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![&self.run_id, &ack.instrument, "", "", 0.0_f64, Some(&ack.deal_reference), Some(status_str), &now],
                )?;
            }
            Ok(())
        });
        if let Err(e) = result {
            eprintln!("  WARN: SQLite batch insert_order (confirm) failed: {e}");
            self.mark_failed();
        }
    }

    /// Record post-trade positions (after execution, for verification).
    pub fn record_post_trade_positions(&self, positions: &HashMap<String, f64>) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                self.mark_failed();
                return;
            }
        };
        let result = db.with_transaction(|conn| {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            for (instrument, &size) in positions {
                conn.execute(
                    "INSERT INTO positions (run_id, instrument, signed_deal_size, source, ts)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![&self.run_id, instrument, size, "post_trade", &now],
                )?;
            }
            Ok(())
        });
        if let Err(e) = result {
            eprintln!("  WARN: SQLite batch insert_position (post_trade) failed: {e}");
            self.mark_failed();
        }
    }

    /// Finalize the run with outcome and duration.
    pub fn record_run_end(&self, outcome: &str, duration_ms: u64) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                self.mark_failed();
                return;
            }
        };
        if let Err(e) = db.finish_run(&self.run_id, outcome, duration_ms) {
            eprintln!("  WARN: SQLite finish_run failed: {e}");
            self.mark_failed();
        }
    }

    /// Record prompt provenance (hash, source, model) on the run row.
    pub fn record_prompt_info(&self, prompt_hash: &str, prompt_source: &str, llm_model: &str) {
        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => {
                eprintln!("  WARN: SQLite lock failed: {e}");
                self.mark_failed();
                return;
            }
        };
        if let Err(e) = db.update_run_prompt(&self.run_id, prompt_hash, prompt_source, llm_model) {
            eprintln!("  WARN: SQLite update_run_prompt failed: {e}");
            self.mark_failed();
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
        let signal_records = vec![
            SignalRecord {
                instrument: "SPY".into(),
                agent_name: "tsmom".into(),
                direction: SignalDirection::Short,
                strength: -0.5,
                confidence: 0.75,
                weight: -0.16,
            },
            SignalRecord {
                instrument: "GLD".into(),
                agent_name: "tsmom".into(),
                direction: SignalDirection::Long,
                strength: 1.0,
                confidence: 1.0,
                weight: 0.40,
            },
        ];
        rec.record_signals(&signal_records);

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

        assert!(!rec.write_failed());

        let db = rec.db.lock().unwrap();
        let runs = db.list_runs(1).unwrap();
        assert_eq!(runs[0].outcome.as_deref(), Some("DRY_RUN"));
    }
}
