use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::config::BlendCategory;

pub mod kronos;
pub mod news;
pub mod volatility;

// ─── Types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayScope {
    Global,
    AssetClass(BlendCategory),
    Instrument(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum OverlayAction {
    FreezeEntries {
        scope: OverlayScope,
        until: NaiveDate,
    },
    ScaleExposure {
        scope: OverlayScope,
        factor: f64,
        until: NaiveDate,
    },
    Flatten {
        scope: OverlayScope,
        reason: String,
    },
    DisableInstrument {
        instrument: String,
        until: NaiveDate,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct AppliedOverlay {
    pub action: OverlayAction,
    pub instruments_affected: Vec<String>,
    pub weight_changes: Vec<(String, f64, f64)>, // (sym, before, after)
}

// ─── Scope Matching ─────────────────────────────────────────────

/// Map instrument symbol to blend category (same logic as combiner::blend_category).
pub(crate) fn symbol_category(symbol: &str) -> BlendCategory {
    match symbol {
        "GLD" | "GC=F" => BlendCategory::Gold,
        "SPY" => BlendCategory::Equity,
        s if s.ends_with("=X") => BlendCategory::Forex,
        _ => BlendCategory::Equity,
    }
}

fn scope_matches(scope: &OverlayScope, instrument: &str) -> bool {
    match scope {
        OverlayScope::Global => true,
        OverlayScope::AssetClass(cat) => symbol_category(instrument) == *cat,
        OverlayScope::Instrument(sym) => sym == instrument,
    }
}

// ─── Core Logic ─────────────────────────────────────────────────

/// Apply overlay actions to the weights map, returning a record of each applied overlay.
///
/// Actions are applied in order. Expired date-bounded actions are skipped.
/// `Flatten` has no expiry and always applies.
pub fn apply_overlays(
    weights: &mut HashMap<String, f64>,
    current_quantities: &HashMap<String, f64>,
    overlays: &[OverlayAction],
    eval_date: NaiveDate,
) -> Vec<AppliedOverlay> {
    let mut applied = Vec::new();

    for action in overlays {
        match action {
            OverlayAction::FreezeEntries { scope, until } => {
                if *until < eval_date {
                    continue; // expired
                }
                let mut affected = Vec::new();
                let mut changes = Vec::new();
                for (sym, weight) in weights.iter_mut() {
                    if !scope_matches(scope, sym) {
                        continue;
                    }
                    let current_qty = current_quantities.get(sym).copied().unwrap_or(0.0);
                    if current_qty == 0.0 {
                        let before = *weight;
                        *weight = 0.0;
                        if before != 0.0 {
                            affected.push(sym.clone());
                            changes.push((sym.clone(), before, 0.0));
                        }
                    }
                }
                if !affected.is_empty() {
                    applied.push(AppliedOverlay {
                        action: action.clone(),
                        instruments_affected: affected,
                        weight_changes: changes,
                    });
                }
            }

            OverlayAction::ScaleExposure {
                scope,
                factor,
                until,
            } => {
                if *until < eval_date {
                    continue; // expired
                }
                let clamped = factor.clamp(0.0, 1.0);
                let mut affected = Vec::new();
                let mut changes = Vec::new();
                for (sym, weight) in weights.iter_mut() {
                    if !scope_matches(scope, sym) {
                        continue;
                    }
                    let before = *weight;
                    *weight *= clamped;
                    if before != *weight {
                        affected.push(sym.clone());
                        changes.push((sym.clone(), before, *weight));
                    }
                }
                if !affected.is_empty() {
                    applied.push(AppliedOverlay {
                        action: action.clone(),
                        instruments_affected: affected,
                        weight_changes: changes,
                    });
                }
            }

            OverlayAction::Flatten { scope, .. } => {
                let mut affected = Vec::new();
                let mut changes = Vec::new();
                for (sym, weight) in weights.iter_mut() {
                    if !scope_matches(scope, sym) {
                        continue;
                    }
                    let before = *weight;
                    *weight = 0.0;
                    if before != 0.0 {
                        affected.push(sym.clone());
                        changes.push((sym.clone(), before, 0.0));
                    }
                }
                if !affected.is_empty() {
                    applied.push(AppliedOverlay {
                        action: action.clone(),
                        instruments_affected: affected,
                        weight_changes: changes,
                    });
                }
            }

            OverlayAction::DisableInstrument { instrument, until } => {
                if *until < eval_date {
                    continue; // expired
                }
                if let Some(weight) = weights.get_mut(instrument) {
                    let before = *weight;
                    *weight = 0.0;
                    if before != 0.0 {
                        applied.push(AppliedOverlay {
                            action: action.clone(),
                            instruments_affected: vec![instrument.clone()],
                            weight_changes: vec![(instrument.clone(), before, 0.0)],
                        });
                    }
                }
            }
        }
    }

    applied
}

// ─── Dedup ──────────────────────────────────────────────────────

/// Canonical scope string for dedup keys.
fn scope_canonical(scope: &OverlayScope) -> String {
    match scope {
        OverlayScope::Global => "global".to_string(),
        OverlayScope::AssetClass(cat) => format!("asset_class:{cat}"),
        OverlayScope::Instrument(sym) => format!("instrument:{}", sym.to_uppercase()),
    }
}

/// Compute a canonical dedup key for an overlay action.
pub fn dedup_key(action: &OverlayAction) -> String {
    match action {
        OverlayAction::FreezeEntries { scope, until } => {
            format!("freeze|{}|{}", scope_canonical(scope), until)
        }
        OverlayAction::ScaleExposure {
            scope,
            factor,
            until,
        } => {
            format!("scale|{}|{:.4}|{}", scope_canonical(scope), factor, until)
        }
        OverlayAction::Flatten { scope, reason } => {
            format!("flatten|{}|{}", scope_canonical(scope), reason)
        }
        OverlayAction::DisableInstrument { instrument, until } => {
            format!("disable|{}|{}", instrument.to_uppercase(), until)
        }
    }
}

/// Filter out proposed overlay actions whose dedup key is already active.
///
/// Returns `(new_actions, skipped_count)`.
pub fn dedup_actions(
    proposed: Vec<OverlayAction>,
    active_keys: &HashSet<String>,
    eval_date: NaiveDate,
) -> (Vec<OverlayAction>, usize) {
    let mut new = Vec::new();
    let mut skipped = 0;

    for action in proposed {
        let key = dedup_key(&action);

        // Only consider an active key as a dup if the action hasn't expired
        let still_active = action_until(&action)
            .map(|u| u >= eval_date)
            .unwrap_or(true); // Flatten has no expiry

        if active_keys.contains(&key) && still_active {
            skipped += 1;
        } else {
            new.push(action);
        }
    }

    (new, skipped)
}

/// Extract the `until` date from an action (None for Flatten).
fn action_until(action: &OverlayAction) -> Option<NaiveDate> {
    match action {
        OverlayAction::FreezeEntries { until, .. } => Some(*until),
        OverlayAction::ScaleExposure { until, .. } => Some(*until),
        OverlayAction::DisableInstrument { until, .. } => Some(*until),
        OverlayAction::Flatten { .. } => None,
    }
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_weights() -> HashMap<String, f64> {
        let mut w = HashMap::new();
        w.insert("GLD".into(), 0.4);
        w.insert("SPY".into(), -0.2);
        w.insert("GBPUSD=X".into(), 0.3);
        w
    }

    // ─── Dedup Tests ────────────────────────────────────────────

    #[test]
    fn dedup_exact_dup_removed() {
        let action = OverlayAction::ScaleExposure {
            scope: OverlayScope::Global,
            factor: 0.5,
            until: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        };
        let key = dedup_key(&action);
        let mut active = HashSet::new();
        active.insert(key);

        let eval = NaiveDate::from_ymd_opt(2026, 4, 9).unwrap();
        let (new, skipped) = dedup_actions(vec![action], &active, eval);
        assert!(new.is_empty());
        assert_eq!(skipped, 1);
    }

    #[test]
    fn dedup_different_scope_kept() {
        let action_global = OverlayAction::FreezeEntries {
            scope: OverlayScope::Global,
            until: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        };
        let action_gold = OverlayAction::FreezeEntries {
            scope: OverlayScope::AssetClass(BlendCategory::Gold),
            until: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        };
        let mut active = HashSet::new();
        active.insert(dedup_key(&action_global));

        let eval = NaiveDate::from_ymd_opt(2026, 4, 9).unwrap();
        let (new, skipped) = dedup_actions(vec![action_gold], &active, eval);
        assert_eq!(new.len(), 1);
        assert_eq!(skipped, 0);
    }

    #[test]
    fn dedup_different_factor_kept() {
        let action1 = OverlayAction::ScaleExposure {
            scope: OverlayScope::Global,
            factor: 0.5,
            until: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        };
        let action2 = OverlayAction::ScaleExposure {
            scope: OverlayScope::Global,
            factor: 0.3,
            until: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        };
        let mut active = HashSet::new();
        active.insert(dedup_key(&action1));

        let eval = NaiveDate::from_ymd_opt(2026, 4, 9).unwrap();
        let (new, skipped) = dedup_actions(vec![action2], &active, eval);
        assert_eq!(new.len(), 1);
        assert_eq!(skipped, 0);
    }

    #[test]
    fn dedup_empty_active_keeps_all() {
        let action = OverlayAction::FreezeEntries {
            scope: OverlayScope::Global,
            until: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        };
        let active = HashSet::new();
        let eval = NaiveDate::from_ymd_opt(2026, 4, 9).unwrap();
        let (new, skipped) = dedup_actions(vec![action], &active, eval);
        assert_eq!(new.len(), 1);
        assert_eq!(skipped, 0);
    }

    #[test]
    fn dedup_expired_dup_not_filtered() {
        // Active key exists, but the proposed action's until is in the past
        let action = OverlayAction::ScaleExposure {
            scope: OverlayScope::Global,
            factor: 0.5,
            until: NaiveDate::from_ymd_opt(2026, 4, 5).unwrap(),
        };
        let key = dedup_key(&action);
        let mut active = HashSet::new();
        active.insert(key);

        let eval = NaiveDate::from_ymd_opt(2026, 4, 9).unwrap();
        let (new, skipped) = dedup_actions(vec![action], &active, eval);
        assert_eq!(new.len(), 1); // expired → not a dup
        assert_eq!(skipped, 0);
    }

    #[test]
    fn test_scale_exposure_global() {
        let mut weights = make_weights();
        let actions = vec![OverlayAction::ScaleExposure {
            scope: OverlayScope::Global,
            factor: 0.5,
            until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        }];
        let applied = apply_overlays(
            &mut weights,
            &HashMap::new(),
            &actions,
            NaiveDate::from_ymd_opt(2026, 4, 8).unwrap(),
        );

        assert_eq!(applied.len(), 1);
        assert!((weights["GLD"] - 0.2).abs() < 1e-10);
        assert!((weights["SPY"] - (-0.1)).abs() < 1e-10);
        assert!((weights["GBPUSD=X"] - 0.15).abs() < 1e-10);
    }

    #[test]
    fn test_scale_exposure_asset_class() {
        let mut weights = make_weights();
        let actions = vec![OverlayAction::ScaleExposure {
            scope: OverlayScope::AssetClass(BlendCategory::Gold),
            factor: 0.5,
            until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        }];
        let applied = apply_overlays(
            &mut weights,
            &HashMap::new(),
            &actions,
            NaiveDate::from_ymd_opt(2026, 4, 8).unwrap(),
        );

        assert_eq!(applied.len(), 1);
        assert!((weights["GLD"] - 0.2).abs() < 1e-10);
        // SPY and GBPUSD=X unchanged
        assert!((weights["SPY"] - (-0.2)).abs() < 1e-10);
        assert!((weights["GBPUSD=X"] - 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_freeze_entries_no_position() {
        let mut weights = make_weights();
        let actions = vec![OverlayAction::FreezeEntries {
            scope: OverlayScope::Global,
            until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        }];
        // No current positions → all weights zeroed
        let applied = apply_overlays(
            &mut weights,
            &HashMap::new(),
            &actions,
            NaiveDate::from_ymd_opt(2026, 4, 8).unwrap(),
        );

        assert_eq!(applied.len(), 1);
        assert_eq!(weights["GLD"], 0.0);
        assert_eq!(weights["SPY"], 0.0);
        assert_eq!(weights["GBPUSD=X"], 0.0);
    }

    #[test]
    fn test_freeze_entries_existing_position() {
        let mut weights = make_weights();
        let mut current = HashMap::new();
        current.insert("GLD".into(), 100.0); // has position
                                             // SPY and GBPUSD=X have no position

        let actions = vec![OverlayAction::FreezeEntries {
            scope: OverlayScope::Global,
            until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        }];
        let applied = apply_overlays(
            &mut weights,
            &current,
            &actions,
            NaiveDate::from_ymd_opt(2026, 4, 8).unwrap(),
        );

        assert_eq!(applied.len(), 1);
        // GLD preserved (has position)
        assert!((weights["GLD"] - 0.4).abs() < 1e-10);
        // Others zeroed
        assert_eq!(weights["SPY"], 0.0);
        assert_eq!(weights["GBPUSD=X"], 0.0);
    }

    #[test]
    fn test_flatten_sets_zero() {
        let mut weights = make_weights();
        let actions = vec![OverlayAction::Flatten {
            scope: OverlayScope::AssetClass(BlendCategory::Forex),
            reason: "weekend risk".into(),
        }];
        let applied = apply_overlays(
            &mut weights,
            &HashMap::new(),
            &actions,
            NaiveDate::from_ymd_opt(2026, 4, 8).unwrap(),
        );

        assert_eq!(applied.len(), 1);
        assert_eq!(weights["GBPUSD=X"], 0.0);
        // Others unchanged
        assert!((weights["GLD"] - 0.4).abs() < 1e-10);
        assert!((weights["SPY"] - (-0.2)).abs() < 1e-10);
    }

    #[test]
    fn test_disable_instrument() {
        let mut weights = make_weights();
        let actions = vec![OverlayAction::DisableInstrument {
            instrument: "SPY".into(),
            until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        }];
        let applied = apply_overlays(
            &mut weights,
            &HashMap::new(),
            &actions,
            NaiveDate::from_ymd_opt(2026, 4, 8).unwrap(),
        );

        assert_eq!(applied.len(), 1);
        assert_eq!(weights["SPY"], 0.0);
        assert!((weights["GLD"] - 0.4).abs() < 1e-10);
    }

    #[test]
    fn test_expired_action_skipped() {
        let mut weights = make_weights();
        let actions = vec![OverlayAction::ScaleExposure {
            scope: OverlayScope::Global,
            factor: 0.5,
            until: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(), // before eval_date
        }];
        let applied = apply_overlays(
            &mut weights,
            &HashMap::new(),
            &actions,
            NaiveDate::from_ymd_opt(2026, 4, 8).unwrap(),
        );

        assert!(applied.is_empty());
        // Weights unchanged
        assert!((weights["GLD"] - 0.4).abs() < 1e-10);
    }

    #[test]
    fn test_multiple_overlays_compose() {
        let mut weights = make_weights();
        let mut current = HashMap::new();
        current.insert("GLD".into(), 100.0);

        let actions = vec![
            // Scale gold by 0.5
            OverlayAction::ScaleExposure {
                scope: OverlayScope::AssetClass(BlendCategory::Gold),
                factor: 0.5,
                until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            },
            // Freeze entries (no new positions)
            OverlayAction::FreezeEntries {
                scope: OverlayScope::Global,
                until: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            },
        ];
        let applied = apply_overlays(
            &mut weights,
            &current,
            &actions,
            NaiveDate::from_ymd_opt(2026, 4, 8).unwrap(),
        );

        assert_eq!(applied.len(), 2);
        // GLD: scaled to 0.2, then freeze skipped (has position)
        assert!((weights["GLD"] - 0.2).abs() < 1e-10);
        // SPY: not scaled (not gold), then frozen (no position)
        assert_eq!(weights["SPY"], 0.0);
        // GBPUSD=X: not scaled, then frozen
        assert_eq!(weights["GBPUSD=X"], 0.0);
    }
}
