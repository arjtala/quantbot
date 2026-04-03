use std::path::Path;

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, Result as SqlResult};

// ─── Schema ─────────────────────────────────────────────────────

/// Schema version stored in PRAGMA user_version. Bump when making breaking
/// changes to the table layout.
pub const DB_SCHEMA_VERSION: i32 = 2;

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS runs (
    run_id      TEXT PRIMARY KEY,
    started_at  TEXT NOT NULL,
    config_json TEXT NOT NULL,
    nav_usd     REAL NOT NULL,
    outcome     TEXT,
    duration_ms INTEGER
);

CREATE TABLE IF NOT EXISTS signals (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      TEXT NOT NULL REFERENCES runs(run_id),
    instrument  TEXT NOT NULL,
    direction   TEXT NOT NULL,
    strength    REAL NOT NULL,
    confidence  REAL NOT NULL,
    weight      REAL NOT NULL,
    ts          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS orders (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id          TEXT NOT NULL REFERENCES runs(run_id),
    instrument      TEXT NOT NULL,
    epic            TEXT NOT NULL,
    direction       TEXT NOT NULL,
    size            REAL NOT NULL,
    deal_reference  TEXT,
    status          TEXT,
    ts              TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS positions (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id              TEXT NOT NULL REFERENCES runs(run_id),
    instrument          TEXT NOT NULL,
    signed_deal_size    REAL NOT NULL,
    source              TEXT NOT NULL,
    ts                  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_signals_run ON signals(run_id);
CREATE INDEX IF NOT EXISTS idx_orders_run ON orders(run_id);
CREATE INDEX IF NOT EXISTS idx_positions_run ON positions(run_id);
CREATE INDEX IF NOT EXISTS idx_orders_instrument ON orders(instrument);
CREATE INDEX IF NOT EXISTS idx_positions_instrument ON positions(instrument);
CREATE INDEX IF NOT EXISTS idx_runs_started_at ON runs(started_at);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
";

/// SQL for the risk_state table added in schema v2.
const RISK_STATE_SQL: &str = "
CREATE TABLE IF NOT EXISTS risk_state (
    key         TEXT PRIMARY KEY,
    value       REAL NOT NULL,
    updated_at  TEXT NOT NULL
);
";

// ─── Database ───────────────────────────────────────────────────

/// SQLite database for recording trading activity across runs.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open (or create) the database at the given path and ensure schema exists.
    pub fn open(path: &Path) -> SqlResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")?;
        conn.execute_batch(SCHEMA_SQL)?;
        Self::check_schema_version(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (for tests).
    #[cfg(test)]
    pub fn open_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA_SQL)?;
        Self::check_schema_version(&conn)?;
        Ok(Self { conn })
    }

    /// Check and set PRAGMA user_version for schema migration tracking.
    fn check_schema_version(conn: &Connection) -> SqlResult<()> {
        let current: i32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
        if current == 0 {
            // Fresh database — create all tables and stamp with current version
            conn.execute_batch(RISK_STATE_SQL)?;
            conn.execute_batch(&format!("PRAGMA user_version = {};", DB_SCHEMA_VERSION))?;
        } else if current < DB_SCHEMA_VERSION {
            // Run migrations
            if current < 2 {
                conn.execute_batch(RISK_STATE_SQL)?;
            }
            conn.execute_batch(&format!("PRAGMA user_version = {};", DB_SCHEMA_VERSION))?;
            eprintln!("  DB migrated from schema v{current} to v{DB_SCHEMA_VERSION}");
        } else if current > DB_SCHEMA_VERSION {
            eprintln!(
                "  WARN: DB schema version {current} is newer than expected {DB_SCHEMA_VERSION}; compatibility not guaranteed"
            );
        }
        Ok(())
    }

    /// Return the schema version stored in the database.
    pub fn schema_version(&self) -> SqlResult<i32> {
        self.conn.query_row("PRAGMA user_version;", [], |row| row.get(0))
    }

    // ── Inserts ─────────────────────────────────────────────────

    /// Run a closure inside a transaction. Commits on Ok, rolls back on Err.
    pub fn with_transaction<F, T>(&self, f: F) -> SqlResult<T>
    where
        F: FnOnce(&Connection) -> SqlResult<T>,
    {
        let tx = self.conn.unchecked_transaction()?;
        let result = f(&self.conn);
        match &result {
            Ok(_) => tx.commit()?,
            Err(_) => tx.rollback()?,
        }
        result
    }

    pub fn insert_run(
        &self,
        run_id: &str,
        config_json: &str,
        nav_usd: f64,
    ) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true);
        self.conn.execute(
            "INSERT INTO runs (run_id, started_at, config_json, nav_usd) VALUES (?1, ?2, ?3, ?4)",
            params![run_id, now, config_json, nav_usd],
        )?;
        Ok(())
    }

    pub fn finish_run(
        &self,
        run_id: &str,
        outcome: &str,
        duration_ms: u64,
    ) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE runs SET outcome = ?1, duration_ms = ?2 WHERE run_id = ?3",
            params![outcome, duration_ms as i64, run_id],
        )?;
        Ok(())
    }

    pub fn insert_signal(
        &self,
        run_id: &str,
        instrument: &str,
        direction: &str,
        strength: f64,
        confidence: f64,
        weight: f64,
    ) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true);
        self.conn.execute(
            "INSERT INTO signals (run_id, instrument, direction, strength, confidence, weight, ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![run_id, instrument, direction, strength, confidence, weight, now],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_order(
        &self,
        run_id: &str,
        instrument: &str,
        epic: &str,
        direction: &str,
        size: f64,
        deal_reference: Option<&str>,
        status: Option<&str>,
    ) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true);
        self.conn.execute(
            "INSERT INTO orders (run_id, instrument, epic, direction, size, deal_reference, status, ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![run_id, instrument, epic, direction, size, deal_reference, status, now],
        )?;
        Ok(())
    }

    pub fn insert_position(
        &self,
        run_id: &str,
        instrument: &str,
        signed_deal_size: f64,
        source: &str,
    ) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true);
        self.conn.execute(
            "INSERT INTO positions (run_id, instrument, signed_deal_size, source, ts)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_id, instrument, signed_deal_size, source, now],
        )?;
        Ok(())
    }

    // ── Queries ─────────────────────────────────────────────────

    /// List recent runs (most recent first).
    pub fn list_runs(&self, limit: usize) -> SqlResult<Vec<RunRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT run_id, started_at, nav_usd, outcome, duration_ms
             FROM runs ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(RunRow {
                run_id: row.get(0)?,
                started_at: row.get(1)?,
                nav_usd: row.get(2)?,
                outcome: row.get(3)?,
                duration_ms: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// Get orders for a specific run.
    pub fn orders_for_run(&self, run_id: &str) -> SqlResult<Vec<OrderRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT instrument, epic, direction, size, deal_reference, status, ts
             FROM orders WHERE run_id = ?1 ORDER BY ts",
        )?;
        let rows = stmt.query_map(params![run_id], |row| {
            Ok(OrderRow {
                instrument: row.get(0)?,
                epic: row.get(1)?,
                direction: row.get(2)?,
                size: row.get(3)?,
                deal_reference: row.get(4)?,
                status: row.get(5)?,
                ts: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    /// Get orders for a specific instrument across all runs.
    pub fn orders_for_instrument(
        &self,
        instrument: &str,
        limit: usize,
    ) -> SqlResult<Vec<OrderRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT instrument, epic, direction, size, deal_reference, status, ts
             FROM orders WHERE instrument = ?1 ORDER BY ts DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![instrument, limit as i64], |row| {
            Ok(OrderRow {
                instrument: row.get(0)?,
                epic: row.get(1)?,
                direction: row.get(2)?,
                size: row.get(3)?,
                deal_reference: row.get(4)?,
                status: row.get(5)?,
                ts: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    /// Get signals for a specific run.
    pub fn signals_for_run(&self, run_id: &str) -> SqlResult<Vec<SignalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT instrument, direction, strength, confidence, weight, ts
             FROM signals WHERE run_id = ?1 ORDER BY ts",
        )?;
        let rows = stmt.query_map(params![run_id], |row| {
            Ok(SignalRow {
                instrument: row.get(0)?,
                direction: row.get(1)?,
                strength: row.get(2)?,
                confidence: row.get(3)?,
                weight: row.get(4)?,
                ts: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    /// List runs filtered by date prefix (e.g. "2026-04-03").
    pub fn list_runs_by_date(&self, date: &str, limit: usize) -> SqlResult<Vec<RunRow>> {
        let like_pattern = format!("{date}%");
        let mut stmt = self.conn.prepare(
            "SELECT run_id, started_at, nav_usd, outcome, duration_ms
             FROM runs WHERE started_at LIKE ?1 ORDER BY started_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![like_pattern, limit as i64], |row| {
            Ok(RunRow {
                run_id: row.get(0)?,
                started_at: row.get(1)?,
                nav_usd: row.get(2)?,
                outcome: row.get(3)?,
                duration_ms: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// Get orders for a run, optionally filtered by status.
    pub fn orders_for_run_filtered(
        &self,
        run_id: &str,
        status: Option<&str>,
    ) -> SqlResult<Vec<OrderRow>> {
        match status {
            Some(status) => {
                let mut stmt = self.conn.prepare(
                    "SELECT instrument, epic, direction, size, deal_reference, status, ts
                     FROM orders WHERE run_id = ?1 AND status = ?2 ORDER BY ts",
                )?;
                let rows = stmt.query_map(params![run_id, status], |row| {
                    Ok(OrderRow {
                        instrument: row.get(0)?,
                        epic: row.get(1)?,
                        direction: row.get(2)?,
                        size: row.get(3)?,
                        deal_reference: row.get(4)?,
                        status: row.get(5)?,
                        ts: row.get(6)?,
                    })
                })?;
                rows.collect()
            }
            None => self.orders_for_run(run_id),
        }
    }

    // ── Risk State ────────────────────────────────────────────────

    /// Get the peak NAV from the risk_state table. Returns None if not yet set.
    pub fn get_peak_nav(&self) -> SqlResult<Option<f64>> {
        let result = self.conn.query_row(
            "SELECT value FROM risk_state WHERE key = 'peak_nav'",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Update the peak NAV in the risk_state table (upsert).
    pub fn update_peak_nav(&self, peak_nav: f64) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true);
        self.conn.execute(
            "INSERT INTO risk_state (key, value, updated_at) VALUES ('peak_nav', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?1, updated_at = ?2",
            params![peak_nav, now],
        )?;
        Ok(())
    }
}

// ─── Row Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct RunRow {
    pub run_id: String,
    pub started_at: String,
    pub nav_usd: f64,
    pub outcome: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OrderRow {
    pub instrument: String,
    pub epic: String,
    pub direction: String,
    pub size: f64,
    pub deal_reference: Option<String>,
    pub status: Option<String>,
    pub ts: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SignalRow {
    pub instrument: String,
    pub direction: String,
    pub strength: f64,
    pub confidence: f64,
    pub weight: f64,
    pub ts: String,
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_creation() {
        let db = Db::open_memory().unwrap();
        // Should be able to insert and query without errors
        db.insert_run("test-run-1", "{}", 1_000_000.0).unwrap();
        let runs = db.list_runs(10).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "test-run-1");
        assert!(runs[0].outcome.is_none());
    }

    #[test]
    fn insert_and_query_orders() {
        let db = Db::open_memory().unwrap();
        db.insert_run("run-1", "{}", 1e6).unwrap();
        db.insert_order("run-1", "SPY", "IX.D.SPTRD.DAILY.IP", "SELL", 282.0, Some("REF1"), Some("Accepted"))
            .unwrap();
        db.insert_order("run-1", "GLD", "UC.D.GLDUS.DAILY.IP", "BUY", 1388.0, Some("REF2"), Some("Accepted"))
            .unwrap();

        let orders = db.orders_for_run("run-1").unwrap();
        assert_eq!(orders.len(), 2);

        let spy_orders = db.orders_for_instrument("SPY", 10).unwrap();
        assert_eq!(spy_orders.len(), 1);
        assert_eq!(spy_orders[0].size, 282.0);
    }

    #[test]
    fn insert_and_query_signals() {
        let db = Db::open_memory().unwrap();
        db.insert_run("run-1", "{}", 1e6).unwrap();
        db.insert_signal("run-1", "GBPUSD=X", "Long", 0.5, 0.75, 0.33)
            .unwrap();

        let signals = db.signals_for_run("run-1").unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].instrument, "GBPUSD=X");
        assert_eq!(signals[0].strength, 0.5);
    }

    #[test]
    fn insert_positions() {
        let db = Db::open_memory().unwrap();
        db.insert_run("run-1", "{}", 1e6).unwrap();
        db.insert_position("run-1", "GC=F", 128.0, "target").unwrap();
        db.insert_position("run-1", "GC=F", 128.0, "actual").unwrap();
    }

    #[test]
    fn finish_run() {
        let db = Db::open_memory().unwrap();
        db.insert_run("run-1", "{}", 1e6).unwrap();
        db.finish_run("run-1", "SUCCESS", 4500).unwrap();

        let runs = db.list_runs(1).unwrap();
        assert_eq!(runs[0].outcome.as_deref(), Some("SUCCESS"));
        assert_eq!(runs[0].duration_ms, Some(4500));
    }

    #[test]
    fn multiple_runs_ordered() {
        let db = Db::open_memory().unwrap();
        db.insert_run("run-1", "{}", 1e6).unwrap();
        db.insert_run("run-2", "{}", 1e6).unwrap();

        let runs = db.list_runs(10).unwrap();
        assert_eq!(runs.len(), 2);
        // Most recent first (run-2 has later started_at)
        assert_eq!(runs[0].run_id, "run-2");
    }

    #[test]
    fn open_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nested/deep/quantbot.db");
        let db = Db::open(&db_path).unwrap();
        db.insert_run("test", "{}", 1e6).unwrap();
        assert!(db_path.exists());
    }

    #[test]
    fn schema_version_set_on_fresh_db() {
        let db = Db::open_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), DB_SCHEMA_VERSION);
        assert_eq!(DB_SCHEMA_VERSION, 2);
    }

    #[test]
    fn list_runs_by_date() {
        let db = Db::open_memory().unwrap();
        db.insert_run("run-1", "{}", 1e6).unwrap();
        db.insert_run("run-2", "{}", 1e6).unwrap();

        // started_at is set to Utc::now() which contains today's date
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let runs = db.list_runs_by_date(&today, 10).unwrap();
        assert_eq!(runs.len(), 2);

        // Non-matching date returns empty
        let runs = db.list_runs_by_date("1999-01-01", 10).unwrap();
        assert!(runs.is_empty());
    }

    #[test]
    fn orders_for_run_filtered_by_status() {
        let db = Db::open_memory().unwrap();
        db.insert_run("run-1", "{}", 1e6).unwrap();
        db.insert_order("run-1", "SPY", "EPIC", "SELL", 100.0, Some("REF1"), Some("Accepted")).unwrap();
        db.insert_order("run-1", "GLD", "EPIC", "BUY", 50.0, Some("REF2"), Some("Rejected")).unwrap();

        // All orders
        let all = db.orders_for_run_filtered("run-1", None).unwrap();
        assert_eq!(all.len(), 2);

        // Only accepted
        let accepted = db.orders_for_run_filtered("run-1", Some("Accepted")).unwrap();
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].instrument, "SPY");

        // Only rejected
        let rejected = db.orders_for_run_filtered("run-1", Some("Rejected")).unwrap();
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].instrument, "GLD");
    }

    #[test]
    fn with_transaction_commits_on_success() {
        let db = Db::open_memory().unwrap();
        db.insert_run("run-tx", "{}", 1e6).unwrap();
        let result = db.with_transaction(|conn| {
            conn.execute(
                "INSERT INTO signals (run_id, instrument, direction, strength, confidence, weight, ts)
                 VALUES ('run-tx', 'A', 'Long', 1.0, 1.0, 0.5, 'now')",
                [],
            )?;
            conn.execute(
                "INSERT INTO signals (run_id, instrument, direction, strength, confidence, weight, ts)
                 VALUES ('run-tx', 'B', 'Short', -1.0, 0.8, 0.3, 'now')",
                [],
            )?;
            Ok(())
        });
        assert!(result.is_ok());
        let signals = db.signals_for_run("run-tx").unwrap();
        assert_eq!(signals.len(), 2);
    }

    #[test]
    fn peak_nav_lifecycle() {
        let db = Db::open_memory().unwrap();

        // Initially no peak_nav
        assert!(db.get_peak_nav().unwrap().is_none());

        // Set initial peak
        db.update_peak_nav(1_000_000.0).unwrap();
        assert_eq!(db.get_peak_nav().unwrap(), Some(1_000_000.0));

        // Update to higher value
        db.update_peak_nav(1_050_000.0).unwrap();
        assert_eq!(db.get_peak_nav().unwrap(), Some(1_050_000.0));
    }

    #[test]
    fn schema_version_is_2() {
        let db = Db::open_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), 2);
    }
}
