use std::path::Path;

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, Result as SqlResult};

// ─── Schema ─────────────────────────────────────────────────────

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
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (for tests).
    #[cfg(test)]
    pub fn open_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self { conn })
    }

    // ── Inserts ─────────────────────────────────────────────────

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
}
