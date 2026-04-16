use std::path::Path;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::config::{BlendCategory, NewsOverlayConfig};
use crate::overlay::{OverlayAction, OverlayScope};

// ─── Feed Types ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsFeed {
    pub events: Vec<NewsEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsEvent {
    /// ISO date when this event applies (point-in-time).
    pub date: NaiveDate,
    /// Scope of the action.
    pub scope: ScopeSpec,
    /// Severity label (for logging/display).
    pub severity: Severity,
    /// Which overlay action to emit.
    pub action: ActionSpec,
    /// How many days the action persists from the event date.
    #[serde(default = "default_until_days")]
    pub until_days: u32,
    /// Human-readable reason (logged to audit).
    pub reason: String,
}

fn default_until_days() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeSpec {
    Global,
    AssetClass(BlendCategory),
    Instrument(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ActionSpec {
    FreezeEntries,
    ScaleExposure {
        #[serde(default = "default_scale_factor")]
        factor: f64,
    },
    Flatten,
    DisableInstrument {
        instrument: String,
    },
}

fn default_scale_factor() -> f64 {
    0.5
}

// ─── Trigger Result (for audit/display) ─────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct NewsTrigger {
    pub date: NaiveDate,
    pub scope: String,
    pub severity: String,
    pub action: String,
    pub reason: String,
    pub until: NaiveDate,
}

// ─── Feed Loading ───────────────────────────────────────────────

pub fn load_feed(path: &Path) -> Result<NewsFeed> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read news feed: {}", path.display()))?;
    let feed: NewsFeed =
        serde_json::from_str(&contents).with_context(|| "failed to parse news feed JSON")?;

    // Validate events: warn on suspect values
    for (i, event) in feed.events.iter().enumerate() {
        if let ActionSpec::ScaleExposure { factor } = &event.action {
            if !(0.0..=1.0).contains(factor) {
                eprintln!(
                    "  WARN: news feed event[{i}]: scale_exposure factor {factor} outside [0,1], will be clamped"
                );
            }
        }
        if event.reason.is_empty() {
            eprintln!("  WARN: news feed event[{i}]: missing reason");
        }
    }

    Ok(feed)
}

// ─── Scope Conversion ───────────────────────────────────────────

fn scope_to_overlay(spec: &ScopeSpec) -> OverlayScope {
    match spec {
        ScopeSpec::Global => OverlayScope::Global,
        ScopeSpec::AssetClass(cat) => OverlayScope::AssetClass(*cat),
        ScopeSpec::Instrument(sym) => OverlayScope::Instrument(sym.clone()),
    }
}

fn scope_label(spec: &ScopeSpec) -> String {
    match spec {
        ScopeSpec::Global => "global".to_string(),
        ScopeSpec::AssetClass(cat) => format!("asset_class:{cat}"),
        ScopeSpec::Instrument(sym) => format!("instrument:{sym}"),
    }
}

// ─── Core Logic ─────────────────────────────────────────────────

/// Compute news overlay actions from a feed file.
///
/// News overlays are **daily**: events apply on that calendar date
/// (`event.date == eval_date`). Intraday timestamps and timezone
/// considerations are out of scope for v1 — a headline at 23:50
/// applies on the same calendar date, not the next day.
///
/// Each matched event is converted into an `OverlayAction` with
/// `until = eval_date + until_days` (falls back to config default
/// when `until_days == 0`).
pub fn compute_news_actions(
    feed: &NewsFeed,
    eval_date: NaiveDate,
    cfg: &NewsOverlayConfig,
) -> (Vec<OverlayAction>, Vec<NewsTrigger>) {
    if !cfg.enabled {
        return (vec![], vec![]);
    }

    let mut actions = Vec::new();
    let mut triggers = Vec::new();

    for event in &feed.events {
        if event.date != eval_date {
            continue;
        }

        let until_days = if event.until_days > 0 {
            event.until_days
        } else {
            cfg.default_until_days
        };
        let until = eval_date + chrono::Duration::days(until_days as i64);
        let scope = scope_to_overlay(&event.scope);

        let action = match &event.action {
            ActionSpec::FreezeEntries => OverlayAction::FreezeEntries { scope, until },
            ActionSpec::ScaleExposure { factor } => OverlayAction::ScaleExposure {
                scope,
                factor: factor.clamp(0.0, 1.0),
                until,
            },
            ActionSpec::Flatten => OverlayAction::Flatten {
                scope,
                reason: event.reason.clone(),
            },
            ActionSpec::DisableInstrument { instrument } => OverlayAction::DisableInstrument {
                instrument: instrument.clone(),
                until,
            },
        };

        let action_label = match &event.action {
            ActionSpec::FreezeEntries => "freeze_entries".to_string(),
            ActionSpec::ScaleExposure { factor } => format!("scale_exposure(factor={factor})"),
            ActionSpec::Flatten => "flatten".to_string(),
            ActionSpec::DisableInstrument { instrument } => {
                format!("disable_instrument({instrument})")
            }
        };

        let severity_label = match event.severity {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        };

        triggers.push(NewsTrigger {
            date: event.date,
            scope: scope_label(&event.scope),
            severity: severity_label.to_string(),
            action: action_label,
            reason: event.reason.clone(),
            until,
        });

        actions.push(action);
    }

    (actions, triggers)
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> NewsOverlayConfig {
        NewsOverlayConfig {
            enabled: true,
            feed_path: "data/news_feed.json".to_string(),
            default_until_days: 1,
        }
    }

    fn sample_feed() -> NewsFeed {
        NewsFeed {
            events: vec![
                NewsEvent {
                    date: NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
                    scope: ScopeSpec::AssetClass(BlendCategory::Gold),
                    severity: Severity::High,
                    action: ActionSpec::FreezeEntries,
                    until_days: 1,
                    reason: "Fed surprise; risk-off".to_string(),
                },
                NewsEvent {
                    date: NaiveDate::from_ymd_opt(2025, 1, 22).unwrap(),
                    scope: ScopeSpec::Instrument("SPY".to_string()),
                    severity: Severity::Medium,
                    action: ActionSpec::ScaleExposure { factor: 0.5 },
                    until_days: 1,
                    reason: "CPI print day".to_string(),
                },
                NewsEvent {
                    date: NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
                    scope: ScopeSpec::Global,
                    severity: Severity::Critical,
                    action: ActionSpec::Flatten,
                    until_days: 0, // uses config default
                    reason: "Black swan event".to_string(),
                },
            ],
        }
    }

    #[test]
    fn parse_feed_json() {
        let json = r#"{
            "events": [
                {
                    "date": "2025-03-15",
                    "scope": "global",
                    "severity": "high",
                    "action": { "type": "freeze_entries" },
                    "until_days": 2,
                    "reason": "FOMC day"
                },
                {
                    "date": "2025-01-22",
                    "scope": { "instrument": "SPY" },
                    "severity": "medium",
                    "action": { "type": "scale_exposure", "factor": 0.5 },
                    "until_days": 1,
                    "reason": "CPI print"
                },
                {
                    "date": "2025-04-01",
                    "scope": { "asset_class": "gold" },
                    "severity": "low",
                    "action": { "type": "flatten" },
                    "reason": "Gold volatility"
                },
                {
                    "date": "2025-04-01",
                    "scope": "global",
                    "severity": "critical",
                    "action": { "type": "disable_instrument", "instrument": "GBPUSD=X" },
                    "reason": "Liquidity gap"
                }
            ]
        }"#;
        let feed: NewsFeed = serde_json::from_str(json).unwrap();
        assert_eq!(feed.events.len(), 4);
        assert!(matches!(feed.events[0].scope, ScopeSpec::Global));
        assert!(matches!(feed.events[1].scope, ScopeSpec::Instrument(_)));
        assert!(matches!(
            feed.events[2].scope,
            ScopeSpec::AssetClass(BlendCategory::Gold)
        ));
        assert!(matches!(
            feed.events[3].action,
            ActionSpec::DisableInstrument { .. }
        ));
    }

    #[test]
    fn only_applies_events_for_eval_date() {
        let feed = sample_feed();
        let cfg = default_cfg();

        // eval_date = 2025-03-15 → should match events[0] and events[2] only
        let eval = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let (actions, triggers) = compute_news_actions(&feed, eval, &cfg);
        assert_eq!(actions.len(), 2);
        assert_eq!(triggers.len(), 2);

        // eval_date = 2025-01-22 → should match events[1] only
        let eval = NaiveDate::from_ymd_opt(2025, 1, 22).unwrap();
        let (actions, triggers) = compute_news_actions(&feed, eval, &cfg);
        assert_eq!(actions.len(), 1);
        assert_eq!(triggers.len(), 1);
    }

    #[test]
    fn no_matching_date_no_actions() {
        let feed = sample_feed();
        let cfg = default_cfg();
        let eval = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        let (actions, triggers) = compute_news_actions(&feed, eval, &cfg);
        assert!(actions.is_empty());
        assert!(triggers.is_empty());
    }

    #[test]
    fn disabled_config_no_actions() {
        let feed = sample_feed();
        let mut cfg = default_cfg();
        cfg.enabled = false;
        let eval = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let (actions, triggers) = compute_news_actions(&feed, eval, &cfg);
        assert!(actions.is_empty());
        assert!(triggers.is_empty());
    }

    #[test]
    fn freeze_entries_produces_correct_action() {
        let feed = NewsFeed {
            events: vec![NewsEvent {
                date: NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
                scope: ScopeSpec::Global,
                severity: Severity::High,
                action: ActionSpec::FreezeEntries,
                until_days: 2,
                reason: "FOMC".to_string(),
            }],
        };
        let cfg = default_cfg();
        let eval = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let (actions, _) = compute_news_actions(&feed, eval, &cfg);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            OverlayAction::FreezeEntries { scope, until } => {
                assert!(matches!(scope, OverlayScope::Global));
                assert_eq!(*until, eval + chrono::Duration::days(2));
            }
            other => panic!("expected FreezeEntries, got {other:?}"),
        }
    }

    #[test]
    fn scale_exposure_produces_correct_action() {
        let feed = NewsFeed {
            events: vec![NewsEvent {
                date: NaiveDate::from_ymd_opt(2025, 1, 22).unwrap(),
                scope: ScopeSpec::Instrument("SPY".to_string()),
                severity: Severity::Medium,
                action: ActionSpec::ScaleExposure { factor: 0.3 },
                until_days: 1,
                reason: "CPI".to_string(),
            }],
        };
        let cfg = default_cfg();
        let eval = NaiveDate::from_ymd_opt(2025, 1, 22).unwrap();
        let (actions, _) = compute_news_actions(&feed, eval, &cfg);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            OverlayAction::ScaleExposure {
                scope,
                factor,
                until,
            } => {
                assert!(matches!(scope, OverlayScope::Instrument(ref s) if s == "SPY"));
                assert!((factor - 0.3).abs() < 1e-10);
                assert_eq!(*until, eval + chrono::Duration::days(1));
            }
            other => panic!("expected ScaleExposure, got {other:?}"),
        }
    }

    #[test]
    fn flatten_produces_correct_action() {
        let feed = NewsFeed {
            events: vec![NewsEvent {
                date: NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
                scope: ScopeSpec::AssetClass(BlendCategory::Forex),
                severity: Severity::Critical,
                action: ActionSpec::Flatten,
                until_days: 0,
                reason: "weekend risk".to_string(),
            }],
        };
        let cfg = default_cfg();
        let eval = NaiveDate::from_ymd_opt(2025, 4, 1).unwrap();
        let (actions, _) = compute_news_actions(&feed, eval, &cfg);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            OverlayAction::Flatten { scope, reason } => {
                assert!(matches!(
                    scope,
                    OverlayScope::AssetClass(BlendCategory::Forex)
                ));
                assert_eq!(reason, "weekend risk");
            }
            other => panic!("expected Flatten, got {other:?}"),
        }
    }

    #[test]
    fn until_days_zero_uses_config_default() {
        let feed = NewsFeed {
            events: vec![NewsEvent {
                date: NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
                scope: ScopeSpec::Global,
                severity: Severity::Low,
                action: ActionSpec::FreezeEntries,
                until_days: 0, // should fall back to config default
                reason: "test".to_string(),
            }],
        };
        let mut cfg = default_cfg();
        cfg.default_until_days = 3;
        let eval = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let (actions, triggers) = compute_news_actions(&feed, eval, &cfg);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            OverlayAction::FreezeEntries { until, .. } => {
                assert_eq!(*until, eval + chrono::Duration::days(3));
            }
            other => panic!("expected FreezeEntries, got {other:?}"),
        }
        assert_eq!(triggers[0].until, eval + chrono::Duration::days(3));
    }

    #[test]
    fn deterministic_same_feed_same_actions() {
        let feed = sample_feed();
        let cfg = default_cfg();
        let eval = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();

        let (actions1, triggers1) = compute_news_actions(&feed, eval, &cfg);
        let (actions2, triggers2) = compute_news_actions(&feed, eval, &cfg);

        assert_eq!(actions1.len(), actions2.len());
        assert_eq!(triggers1.len(), triggers2.len());
        for (t1, t2) in triggers1.iter().zip(triggers2.iter()) {
            assert_eq!(t1.reason, t2.reason);
            assert_eq!(t1.until, t2.until);
        }
    }

    #[test]
    fn trigger_details_populated() {
        let feed = sample_feed();
        let cfg = default_cfg();
        let eval = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let (_, triggers) = compute_news_actions(&feed, eval, &cfg);

        assert_eq!(triggers.len(), 2);
        assert_eq!(triggers[0].severity, "high");
        assert!(triggers[0].action.contains("freeze_entries"));
        assert_eq!(triggers[0].reason, "Fed surprise; risk-off");
        assert_eq!(triggers[1].severity, "critical");
        assert!(triggers[1].action.contains("flatten"));
    }

    #[test]
    fn load_feed_from_file() {
        let dir = std::env::temp_dir().join("quantbot_test_news");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_feed.json");
        let json = r#"{"events": [
            {
                "date": "2025-03-15",
                "scope": "global",
                "severity": "high",
                "action": { "type": "freeze_entries" },
                "until_days": 1,
                "reason": "test"
            }
        ]}"#;
        std::fs::write(&path, json).unwrap();

        let feed = load_feed(&path).unwrap();
        assert_eq!(feed.events.len(), 1);

        // cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn disable_instrument_action() {
        let feed = NewsFeed {
            events: vec![NewsEvent {
                date: NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
                scope: ScopeSpec::Global,
                severity: Severity::High,
                action: ActionSpec::DisableInstrument {
                    instrument: "GBPUSD=X".to_string(),
                },
                until_days: 2,
                reason: "liquidity gap".to_string(),
            }],
        };
        let cfg = default_cfg();
        let eval = NaiveDate::from_ymd_opt(2025, 4, 1).unwrap();
        let (actions, _) = compute_news_actions(&feed, eval, &cfg);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            OverlayAction::DisableInstrument { instrument, until } => {
                assert_eq!(instrument, "GBPUSD=X");
                assert_eq!(*until, eval + chrono::Duration::days(2));
            }
            other => panic!("expected DisableInstrument, got {other:?}"),
        }
    }

    #[test]
    fn scale_factor_clamped_to_unit_range() {
        let feed = NewsFeed {
            events: vec![NewsEvent {
                date: NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
                scope: ScopeSpec::Global,
                severity: Severity::Medium,
                action: ActionSpec::ScaleExposure { factor: 1.5 },
                until_days: 1,
                reason: "out of range".to_string(),
            }],
        };
        let cfg = default_cfg();
        let eval = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let (actions, _) = compute_news_actions(&feed, eval, &cfg);
        match &actions[0] {
            OverlayAction::ScaleExposure { factor, .. } => {
                assert!(
                    (factor - 1.0).abs() < 1e-10,
                    "factor should be clamped to 1.0"
                );
            }
            other => panic!("expected ScaleExposure, got {other:?}"),
        }
    }
}
