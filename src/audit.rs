use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::agents::risk::RiskCheckDetail;
use crate::core::portfolio::OrderSide;
use crate::execution::reconcile::{DustDelta, PositionMismatch};
use crate::execution::traits::{DealStatus, OrderAck, OrderRequest};
use crate::overlay::AppliedOverlay;

// ─── Run ID ─────────────────────────────────────────────────────

/// A run identifier derived from the UTC timestamp at run start.
/// Format: `2026-04-02T133500Z` — used as both the JSONL filename stem and the
/// `run_id` field in every event.
#[derive(Debug, Clone)]
pub struct RunId {
    pub id: String,
    pub started_at: DateTime<Utc>,
}

impl RunId {
    pub fn now() -> Self {
        let now = Utc::now();
        Self {
            id: now.format("%Y-%m-%dT%H%M%SZ").to_string(),
            started_at: now,
        }
    }
}

// ─── Event Payloads ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct TargetEntry {
    pub instrument: String,
    pub signed_deal_size: f64,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderEntry {
    pub instrument: String,
    pub epic: String,
    pub direction: String,
    pub size: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderAckEntry {
    pub instrument: String,
    pub deal_reference: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionEntry {
    pub instrument: String,
    pub signed_deal_size: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MismatchEntry {
    pub instrument: String,
    pub target: f64,
    pub actual: f64,
    pub delta: f64,
}

// ─── Audit Event ────────────────────────────────────────────────

/// Schema version. Bump when event payloads change in breaking ways.
pub const SCHEMA_VERSION: u32 = 2;

/// Top-level JSONL record. One per line in the audit file.
#[derive(Debug, Clone, Serialize)]
pub struct AuditRecord {
    pub schema_version: u32,
    pub ts: String,
    pub run_id: String,
    pub event: String,
    pub level: String,
    pub data: serde_json::Value,
}

// ─── AuditLogger ────────────────────────────────────────────────

/// Append-only JSONL logger for a single live trading run.
///
/// Writes to `data/audit/<run_id>.jsonl`. If any write fails, the failure is
/// logged to stderr and the `write_failed` flag is set — trading continues
/// unaffected.
pub struct AuditLogger {
    run_id: RunId,
    writer: Option<BufWriter<File>>,
    path: PathBuf,
    pub write_failed: bool,
}

impl AuditLogger {
    /// Create a new audit logger. Creates `audit_dir` if it doesn't exist.
    /// If file creation fails, returns a logger with `write_failed = true`
    /// (trading must not be blocked by audit I/O).
    pub fn new(run_id: RunId, audit_dir: &Path) -> Self {
        let filename = format!("{}.jsonl", run_id.id);
        let path = audit_dir.join(&filename);

        let writer = match fs::create_dir_all(audit_dir)
            .and_then(|_| File::create(&path))
        {
            Ok(f) => Some(BufWriter::new(f)),
            Err(e) => {
                eprintln!(
                    "  WARN: failed to create audit log {}: {e}",
                    path.display()
                );
                None
            }
        };

        Self {
            run_id,
            write_failed: writer.is_none(),
            writer,
            path,
        }
    }

    /// Path to the audit JSONL file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The run identifier.
    pub fn run_id(&self) -> &str {
        &self.run_id.id
    }

    /// Append one event record. Never panics; on failure sets `write_failed`
    /// and prints to stderr.
    fn log(&mut self, event: &str, level: &str, data: serde_json::Value) {
        let record = AuditRecord {
            schema_version: SCHEMA_VERSION,
            ts: Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true),
            run_id: self.run_id.id.clone(),
            event: event.to_string(),
            level: level.to_string(),
            data,
        };

        let writer = match self.writer.as_mut() {
            Some(w) => w,
            None => return, // already failed to open
        };

        let line = match serde_json::to_string(&record) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  WARN: audit serialize failed for event '{event}': {e}");
                self.write_failed = true;
                return;
            }
        };

        if let Err(e) = writeln!(writer, "{line}") {
            eprintln!("  WARN: audit write failed for event '{event}': {e}");
            self.write_failed = true;
            return;
        }

        // Flush on ERROR events and run_end for reliability
        if level == "ERROR" || event == "run_end" {
            if let Err(e) = writer.flush() {
                eprintln!("  WARN: audit flush failed: {e}");
                self.write_failed = true;
            }
        }
    }

    // ── Typed event methods ─────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn log_run_start(
        &mut self,
        mode: &str,
        engine: &str,
        dry_run: bool,
        config_path: &str,
        instruments: &[String],
        nav: f64,
        ig_environment: Option<&str>,
        state_file: Option<&str>,
    ) {
        self.log(
            "run_start",
            "INFO",
            serde_json::json!({
                "mode": mode,
                "engine": engine,
                "dry_run": dry_run,
                "config_path": config_path,
                "instruments": instruments,
                "nav_usd": nav,
                "ig_environment": ig_environment,
                "state_file": state_file,
            }),
        );
    }

    pub fn log_auth_ok(&mut self, engine: &str) {
        self.log(
            "auth_ok",
            "INFO",
            serde_json::json!({ "engine": engine }),
        );
    }

    pub fn log_health_check_ok(&mut self) {
        self.log(
            "health_check_ok",
            "INFO",
            serde_json::json!({}),
        );
    }

    pub fn log_prompt_info(
        &mut self,
        prompt_hash: &str,
        prompt_source: &str,
        llm_model: &str,
    ) {
        self.log(
            "prompt_info",
            "INFO",
            serde_json::json!({
                "prompt_hash": prompt_hash,
                "prompt_source": prompt_source,
                "llm_model": llm_model,
            }),
        );
    }

    pub fn log_nav_mtm(&mut self, initial_cash: f64, unrealized_pnl: f64, mtm_nav: f64, positions: &[crate::execution::mtm::MtmPosition]) {
        self.log(
            "nav_mark_to_market",
            "INFO",
            serde_json::json!({
                "initial_cash": (initial_cash * 10.0).round() / 10.0,
                "unrealized_pnl": (unrealized_pnl * 10.0).round() / 10.0,
                "mtm_nav": (mtm_nav * 10.0).round() / 10.0,
                "position_count": positions.len(),
                "positions": positions,
            }),
        );
    }

    pub fn log_risk_check(&mut self, detail: &RiskCheckDetail) {
        let level = if detail.decision == "VETO" { "ERROR" } else { "INFO" };
        self.log(
            "risk_check",
            level,
            serde_json::json!({
                "gross_leverage": detail.gross_leverage,
                "max_position_leverage": detail.max_position_leverage,
                "max_position_instrument": detail.max_position_instrument,
                "drawdown_pct": detail.drawdown_pct,
                "peak_nav": detail.peak_nav,
                "current_nav": detail.current_nav,
                "decision": detail.decision,
                "reason": detail.reason,
            }),
        );
    }

    pub fn log_overlays_applied(&mut self, applied: &[AppliedOverlay]) {
        if applied.is_empty() {
            return;
        }
        let actions: Vec<serde_json::Value> = applied
            .iter()
            .map(|a| {
                serde_json::json!({
                    "action": a.action,
                    "instruments_affected": a.instruments_affected,
                    "weight_changes": a.weight_changes.iter().map(|(sym, before, after)| {
                        serde_json::json!({"instrument": sym, "before": before, "after": after})
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();
        self.log(
            "overlay_applied",
            "INFO",
            serde_json::json!({
                "count": applied.len(),
                "actions": actions,
            }),
        );
    }

    pub fn log_volatility_triggers(
        &mut self,
        triggers: &[crate::overlay::volatility::TriggerResult],
        actions_emitted: usize,
    ) {
        if triggers.is_empty() {
            return;
        }
        let trigger_data: Vec<serde_json::Value> = triggers
            .iter()
            .map(|t| {
                serde_json::json!({
                    "instrument": t.instrument,
                    "category": t.category.to_string(),
                    "vol_ratio": t.vol_ratio,
                    "atr_pct": t.atr_pct,
                    "move_sigma": t.move_sigma,
                })
            })
            .collect();
        self.log(
            "volatility_triggers",
            if actions_emitted > 0 { "WARN" } else { "INFO" },
            serde_json::json!({
                "triggers": trigger_data,
                "actions_emitted": actions_emitted,
            }),
        );
    }

    pub fn log_news_triggers(
        &mut self,
        triggers: &[crate::overlay::news::NewsTrigger],
        actions_emitted: usize,
    ) {
        if triggers.is_empty() {
            return;
        }
        let trigger_data: Vec<serde_json::Value> = triggers
            .iter()
            .map(|t| {
                serde_json::json!({
                    "date": t.date.to_string(),
                    "scope": t.scope,
                    "severity": t.severity,
                    "action": t.action,
                    "reason": t.reason,
                    "until": t.until.to_string(),
                })
            })
            .collect();
        self.log(
            "news_triggers",
            if actions_emitted > 0 { "WARN" } else { "INFO" },
            serde_json::json!({
                "event_count": triggers.len(),
                "actions_emitted": actions_emitted,
                "events": trigger_data,
            }),
        );
    }

    pub fn log_targets(
        &mut self,
        eval_date: &str,
        nav: f64,
        targets: &[TargetEntry],
    ) {
        self.log(
            "targets",
            "INFO",
            serde_json::json!({
                "eval_date": eval_date,
                "nav_usd": nav,
                "targets": targets,
            }),
        );
    }

    pub fn log_positions_fetched(&mut self, positions: &[PositionEntry]) {
        self.log(
            "positions_fetched",
            "INFO",
            serde_json::json!({
                "count": positions.len(),
                "positions": positions,
            }),
        );
    }

    pub fn log_reconcile(
        &mut self,
        orders: &[OrderEntry],
        skipped_dust: &[DustDelta],
    ) {
        self.log(
            "reconcile",
            "INFO",
            serde_json::json!({
                "order_count": orders.len(),
                "orders": orders,
                "dust_count": skipped_dust.len(),
                "skipped_dust": skipped_dust,
            }),
        );
    }

    pub fn log_breaker_check(
        &mut self,
        pass: bool,
        order_count: usize,
        max_order_size: f64,
        reason: Option<&str>,
    ) {
        let level = if pass { "INFO" } else { "ERROR" };
        self.log(
            "breaker_check",
            level,
            serde_json::json!({
                "pass": pass,
                "order_count": order_count,
                "max_order_size": max_order_size,
                "reason": reason,
            }),
        );
    }

    pub fn log_execution_skipped(&mut self, reason: &str, order_count: usize) {
        self.log(
            "execution_skipped",
            "INFO",
            serde_json::json!({
                "reason": reason,
                "order_count": order_count,
            }),
        );
    }

    pub fn log_orders_submitted(&mut self, orders: &[OrderEntry]) {
        self.log(
            "orders_submitted",
            "INFO",
            serde_json::json!({
                "count": orders.len(),
                "orders": orders,
            }),
        );
    }

    pub fn log_orders_confirmed(&mut self, acks: &[OrderAckEntry]) {
        // Check for any rejections
        let has_rejection = acks.iter().any(|a| a.status == "Rejected");
        let level = if has_rejection { "WARN" } else { "INFO" };

        self.log(
            "orders_confirmed",
            level,
            serde_json::json!({
                "count": acks.len(),
                "results": acks,
            }),
        );
    }

    pub fn log_verify(
        &mut self,
        pass: bool,
        mismatches: &[MismatchEntry],
    ) {
        let level = if pass { "INFO" } else { "WARN" };
        self.log(
            "verify",
            level,
            serde_json::json!({
                "pass": pass,
                "mismatch_count": mismatches.len(),
                "mismatches": mismatches,
            }),
        );
    }

    pub fn log_run_end(&mut self, outcome: &str, summary: &RunSummary) {
        let level = match outcome {
            "SUCCESS" | "DRY_RUN" => "INFO",
            _ => "ERROR",
        };
        self.log(
            "run_end",
            level,
            serde_json::json!({
                "outcome": outcome,
                "duration_ms": summary.duration_ms,
                "orders_placed": summary.orders_placed,
                "orders_confirmed": summary.orders_confirmed,
                "orders_rejected": summary.orders_rejected,
                "dust_skipped": summary.dust_skipped,
                "mismatches": summary.mismatches,
                "audit_write_failed": self.write_failed,
                "db_write_failed": summary.db_write_failed,
            }),
        );
    }

    pub fn log_error(&mut self, message: &str) {
        self.log(
            "error",
            "ERROR",
            serde_json::json!({
                "message": message,
            }),
        );
    }
}

// ─── Run Summary ────────────────────────────────────────────────

/// Final summary emitted as the `run_end` data payload and optionally
/// printed to stdout as JSON via `--json`.
#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    pub run_id: String,
    pub outcome: String,
    pub duration_ms: u64,
    pub orders_placed: usize,
    pub orders_confirmed: usize,
    pub orders_rejected: usize,
    pub dust_skipped: usize,
    pub mismatches: usize,
    pub audit_write_failed: bool,
    pub db_write_failed: bool,
    pub audit_path: String,
}

impl std::fmt::Display for RunSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RUN {} outcome={} orders={} confirmed={} rejected={} dust={} mismatches={} duration={:.2}s audit={} db_ok={}",
            self.run_id,
            self.outcome,
            self.orders_placed,
            self.orders_confirmed,
            self.orders_rejected,
            self.dust_skipped,
            self.mismatches,
            self.duration_ms as f64 / 1000.0,
            self.audit_path,
            !self.db_write_failed,
        )
    }
}

// ─── Conversion helpers ─────────────────────────────────────────

pub fn order_requests_to_entries(orders: &[OrderRequest]) -> Vec<OrderEntry> {
    orders
        .iter()
        .map(|o| OrderEntry {
            instrument: o.instrument.clone(),
            epic: o.epic.clone(),
            direction: match o.direction {
                OrderSide::Buy => "BUY".to_string(),
                OrderSide::Sell => "SELL".to_string(),
            },
            size: (o.size * 10.0).round() / 10.0,
        })
        .collect()
}

pub fn order_acks_to_entries(acks: &[OrderAck]) -> Vec<OrderAckEntry> {
    acks.iter()
        .map(|a| OrderAckEntry {
            instrument: a.instrument.clone(),
            deal_reference: a.deal_reference.clone(),
            status: match a.status {
                DealStatus::Accepted => "Accepted".to_string(),
                DealStatus::Rejected => "Rejected".to_string(),
                DealStatus::Pending => "Pending".to_string(),
            },
        })
        .collect()
}

pub fn positions_to_entries(signed: &HashMap<String, f64>) -> Vec<PositionEntry> {
    let mut entries: Vec<_> = signed
        .iter()
        .map(|(sym, qty)| PositionEntry {
            instrument: sym.clone(),
            signed_deal_size: (*qty * 10.0).round() / 10.0,
        })
        .collect();
    entries.sort_by(|a, b| a.instrument.cmp(&b.instrument));
    entries
}

pub fn mismatches_to_entries(mismatches: &[PositionMismatch]) -> Vec<MismatchEntry> {
    mismatches
        .iter()
        .map(|m| MismatchEntry {
            instrument: m.instrument.clone(),
            target: m.target,
            actual: m.actual,
            delta: m.delta,
        })
        .collect()
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_id_format() {
        let rid = RunId::now();
        // Should look like 2026-04-02T133500Z
        assert!(rid.id.ends_with('Z'));
        assert!(rid.id.contains('T'));
        assert_eq!(rid.id.len(), 18); // YYYY-MM-DDTHHMMSSZ
    }

    #[test]
    fn logger_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let rid = RunId::now();
        let expected_path = dir.path().join(format!("{}.jsonl", rid.id));

        let mut logger = AuditLogger::new(rid, dir.path());
        assert!(!logger.write_failed);
        assert!(expected_path.exists());

        // Write an event and verify JSONL
        logger.log_run_start("live", "ig", false, "config.toml", &["SPY".into()], 1e6, Some("DEMO"), Some("data/live-state.json"));

        // Flush
        if let Some(w) = logger.writer.as_mut() {
            w.flush().unwrap();
        }

        let content = fs::read_to_string(&expected_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);

        let record: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(record["schema_version"], 2);
        assert_eq!(record["event"], "run_start");
        assert_eq!(record["level"], "INFO");
        assert_eq!(record["data"]["engine"], "ig");
        assert_eq!(record["data"]["dry_run"], false);
        assert_eq!(record["data"]["ig_environment"], "DEMO");
        assert_eq!(record["data"]["state_file"], "data/live-state.json");
    }

    #[test]
    fn logger_handles_missing_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let deep_path = dir.path().join("nested/audit");
        let rid = RunId::now();

        let logger = AuditLogger::new(rid, &deep_path);
        // Should create nested dirs and succeed
        assert!(!logger.write_failed);
    }

    #[test]
    fn run_summary_display() {
        let summary = RunSummary {
            run_id: "2026-04-02T133500Z".into(),
            outcome: "SUCCESS".into(),
            duration_ms: 12050,
            orders_placed: 3,
            orders_confirmed: 3,
            orders_rejected: 0,
            dust_skipped: 1,
            mismatches: 0,
            audit_write_failed: false,
            db_write_failed: false,
            audit_path: "data/audit/2026-04-02T133500Z.jsonl".into(),
        };
        let s = summary.to_string();
        assert!(s.contains("outcome=SUCCESS"));
        assert!(s.contains("orders=3"));
        assert!(s.contains("duration=12.05s"));
        assert!(s.contains("db_ok=true"));
    }

    #[test]
    fn multiple_events_produce_valid_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let rid = RunId::now();
        let run_id_str = rid.id.clone();
        let mut logger = AuditLogger::new(rid, dir.path());

        logger.log_run_start("live", "ig", true, "config.toml", &["SPY".into()], 1e6, Some("DEMO"), None);
        logger.log_targets("2025-03-31", 1e6, &[TargetEntry {
            instrument: "SPY".into(),
            signed_deal_size: -282.0,
            weight: -0.16,
        }]);
        logger.log_run_end("DRY_RUN", &RunSummary {
            run_id: run_id_str.clone(),
            outcome: "DRY_RUN".into(),
            duration_ms: 500,
            orders_placed: 0,
            orders_confirmed: 0,
            orders_rejected: 0,
            dust_skipped: 0,
            mismatches: 0,
            audit_write_failed: false,
            db_write_failed: false,
            audit_path: format!("data/audit/{run_id_str}.jsonl"),
        });

        // Flush and read
        if let Some(w) = logger.writer.as_mut() {
            w.flush().unwrap();
        }

        let path = dir.path().join(format!("{run_id_str}.jsonl"));
        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);

        // Every line must parse as valid JSON with required fields
        for line in &lines {
            let record: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(record["schema_version"], 2);
            assert!(record["ts"].is_string());
            assert_eq!(record["run_id"], run_id_str);
            assert!(record["event"].is_string());
            assert!(record["level"].is_string());
        }

        // Check event ordering
        let events: Vec<String> = lines
            .iter()
            .map(|l| {
                let r: serde_json::Value = serde_json::from_str(l).unwrap();
                r["event"].as_str().unwrap().to_string()
            })
            .collect();
        assert_eq!(events, vec!["run_start", "targets", "run_end"]);
    }

    #[test]
    fn order_request_conversion() {
        let orders = vec![OrderRequest {
            instrument: "SPY".into(),
            epic: "IX.D.SPTRD.DAILY.IP".into(),
            direction: OrderSide::Sell,
            size: 282.0,
            order_type: crate::execution::traits::OrderType::Market,
            currency_code: "GBP".into(),
            expiry: "DFB".into(),
        }];
        let entries = order_requests_to_entries(&orders);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].direction, "SELL");
        assert_eq!(entries[0].size, 282.0);
    }

    #[test]
    fn order_ack_conversion() {
        let acks = vec![OrderAck {
            deal_reference: "REF123".into(),
            instrument: "SPY".into(),
            status: DealStatus::Accepted,
        }];
        let entries = order_acks_to_entries(&acks);
        assert_eq!(entries[0].status, "Accepted");
    }
}
